use crate::error::PoeError;
use crate::types::*;
use futures_util::Stream;
use futures_util::StreamExt;
use futures_util::future::join_all;
use reqwest::Client;
use reqwest::header::{COOKIE, HeaderMap, HeaderValue};
use serde_json::Value;
use std::path::Path;
use std::pin::Pin;
use tokio_util::io::ReaderStream;
#[cfg(feature = "trace")]
use tracing::{debug, warn};

const POE_GQL_URL: &str = "https://poe.com/api/gql_POST";
const POE_GQL_MODEL_HASH: &str = "b24b2f2f6da147b3345eec1a433ed17b6e1332df97dea47622868f41078a40cc";
const POE_GQL_MODEL_REVISION: &str = "e2acc7025b43e08e88164ba8105273f37fbeaa26";

#[derive(Clone)]
pub struct PoeClient {
    client: Client,
    bot_name: String,
    access_key: String,
    poe_base_url: String,
    poe_file_upload_url: String,
}

impl PoeClient {
    pub fn new(
        bot_name: &str,
        access_key: &str,
        poe_base_url: &str,
        poe_file_upload_url: &str,
    ) -> Self {
        #[cfg(feature = "trace")]
        debug!("建立新的 PoeClient 實例，bot_name: {}", bot_name);

        // 處理 URL 末尾的斜線
        let normalized_base_url = if poe_base_url.ends_with('/') {
            poe_base_url.trim_end_matches('/').to_string()
        } else {
            poe_base_url.to_string()
        };

        let normalized_file_upload_url = if poe_file_upload_url.ends_with('/') {
            poe_file_upload_url.trim_end_matches('/').to_string()
        } else {
            poe_file_upload_url.to_string()
        };

        Self {
            client: Client::new(),
            bot_name: bot_name.to_string(),
            access_key: access_key.to_string(),
            poe_base_url: normalized_base_url,
            poe_file_upload_url: normalized_file_upload_url,
        }
    }

    pub async fn stream_request(
        &self,
        #[cfg(feature = "xml")] mut request: ChatRequest,
        #[cfg(not(feature = "xml"))] request: ChatRequest,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<ChatResponse, PoeError>> + Send>>, PoeError> {
        #[cfg(feature = "trace")]
        debug!("開始串流請求，bot_name: {}", self.bot_name);

        // 當啟用 xml feature 時，自動將工具轉換為 XML 格式
        #[cfg(feature = "xml")]
        {
            if request.tools.is_some() {
                #[cfg(feature = "trace")]
                debug!("檢測到 xml feature 啟用，自動將工具轉換為 XML 格式");

                // 使用 xml 模塊中的方法
                request.append_tools_as_xml();
                request.tools = None; // 清除原始工具定義
            }

            // 如果有工具結果，也需要轉換為 XML 格式並清除原始數據
            if request.tool_results.is_some() {
                #[cfg(feature = "trace")]
                debug!("檢測到 xml feature 啟用，自動將工具結果轉換為 XML 格式");

                // 將工具結果轉換為 XML 格式並附加到訊息末尾
                request.append_tool_results_as_xml();

                // 清除原始的工具調用和結果，因為已經轉換為 XML 格式
                request.tool_calls = None;
                request.tool_results = None;
            }
        }

        let url = format!("{}/bot/{}", self.poe_base_url, self.bot_name);
        #[cfg(feature = "trace")]
        debug!("發送請求至 URL: {}", url);

        #[cfg(feature = "trace")]
        debug!(
            "🔍 發送的完整請求體: {}",
            serde_json::to_string_pretty(&request).unwrap_or_else(|_| "無法序列化".to_string())
        );

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.access_key))
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            #[cfg(feature = "trace")]
            warn!("API 請求失敗，狀態碼: {}", status);
            return Err(PoeError::BotError(format!("API 回應狀態碼: {}", status)));
        }

        #[cfg(feature = "trace")]
        debug!("成功接收到串流回應");

        let mut static_buffer = String::new();
        let mut current_event: Option<ChatEventType> = None;
        let mut is_collecting_data = false;
        // 用於累積 tool_calls 的狀態
        let mut accumulated_tool_calls: Vec<PartialToolCall> = Vec::new();
        let mut tool_calls_complete = false;

        // XML 工具調用緩衝和檢測狀態
        #[cfg(feature = "xml")]
        let mut xml_text_buffer = String::new();
        #[cfg(feature = "xml")]
        let mut xml_detection_active = false;

        let stream = response
            .bytes_stream()
            .map(move |result| {
                result.map_err(PoeError::from).map(|chunk| {
                    let chunk_str = String::from_utf8_lossy(&chunk);
                    #[cfg(feature = "trace")]
                    debug!("處理串流塊，大小: {} 字節", chunk.len());

                    let mut events = Vec::new();
                    // 將新的塊添加到靜態緩衝區
                    static_buffer.push_str(&chunk_str);

                    // 尋找完整的消息
                    while let Some(newline_pos) = static_buffer.find('\n') {
                        let line = static_buffer[..newline_pos].trim().to_string();
                        static_buffer = static_buffer[newline_pos + 1..].to_string();

                        if line.is_empty() {
                            // 重置當前事件狀態，準備處理下一個事件
                            current_event = None;
                            is_collecting_data = false;
                            continue;
                        }

                        if line == ": ping" {
                            #[cfg(feature = "trace")]
                            debug!("收到 ping 訊號");
                            continue;
                        }

                        if line.starts_with("event: ") {
                            let event_name = line.trim_start_matches("event: ").trim();
                            #[cfg(feature = "trace")]
                            debug!("解析事件類型: {}", event_name);

                            let event_type = match event_name {
                                "text" => ChatEventType::Text,
                                "replace_response" => ChatEventType::ReplaceResponse,
                                "json" => ChatEventType::Json,
                                "file" => ChatEventType::File,
                                "done" => ChatEventType::Done,
                                "error" => ChatEventType::Error,
                                _ => {
                                    #[cfg(feature = "trace")]
                                    warn!("收到未知事件類型: {}", event_name);
                                    continue;
                                }
                            };

                            current_event = Some(event_type);
                            is_collecting_data = false;
                            continue;
                        }

                        if line.starts_with("data: ") {
                            let data = line.trim_start_matches("data: ").trim();
                            #[cfg(feature = "trace")]
                            debug!(
                                "收到事件數據: {}",
                                if data.len() > 100 { &data[..100] } else { data }
                            );

                            if let Some(ref event_type) = current_event {
                                match event_type {
                                    ChatEventType::Text | ChatEventType::ReplaceResponse => {
                                        if let Ok(json) = serde_json::from_str::<Value>(data) {
                                            if let Some(text) = json.get("text").and_then(Value::as_str) {
                                                #[cfg(feature = "trace")]
                                                debug!("解析到文本數據，長度: {}", text.len());

                                                // XML 工具調用檢測和緩衝邏輯
                                                #[cfg(feature = "xml")]
                                                {
                                                    // 將文本添加到 XML 緩衝區
                                                    xml_text_buffer.push_str(text);
                                                    // 檢查是否開始 XML 工具調用
                                                    if !xml_detection_active && text.contains("<") {
                                                        xml_detection_active = true;
                                                        #[cfg(feature = "trace")]
                                                        debug!("檢測到 '<' 字符，開始 XML 緩衝");
                                                    }
                                                    // 如果正在檢測 XML，檢查是否有完整的工具調用
                                                    if xml_detection_active {
                                                        let message = ChatMessage {
                                                            role: "assistant".to_string(),
                                                            content: xml_text_buffer.clone(),
                                                            attachments: None,
                                                            content_type: "text/plain".to_string(),
                                                        };
                                                        if message.contains_xml_tool_calls() {
                                                            let tool_calls = message.extract_xml_tool_calls();
                                                            if !tool_calls.is_empty() {
                                                                #[cfg(feature = "trace")]
                                                                debug!("檢測到完整的 XML 工具調用，轉換為標準格式，數量: {}", tool_calls.len());
                                                                // 發送工具調用事件
                                                                events.push(Ok(ChatResponse {
                                                                    event: ChatEventType::Json,
                                                                    data: Some(ChatResponseData::ToolCalls(tool_calls)),
                                                                }));
                                                                // 移除 XML 部分並發送剩餘文本
                                                                let clean_text = Self::remove_xml_tool_calls(&xml_text_buffer);
                                                                if !clean_text.trim().is_empty() {
                                                                    events.push(Ok(ChatResponse {
                                                                        event: event_type.clone(),
                                                                        data: Some(ChatResponseData::Text {
                                                                            text: clean_text,
                                                                        }),
                                                                    }));
                                                                }
                                                                // 重置 XML 緩衝狀態
                                                                xml_text_buffer.clear();
                                                                xml_detection_active = false;
                                                            } else {
                                                                // 沒有完整的工具調用，繼續緩衝
                                                                #[cfg(feature = "trace")]
                                                                debug!("XML 工具調用尚未完整，繼續緩衝");
                                                            }
                                                        } else {
                                                            // 檢查緩衝區是否過大或包含結束標記
                                                            if xml_text_buffer.len() > 10000 ||
                                                               (!xml_text_buffer.contains("<tool_call>") &&
                                                                !xml_text_buffer.contains("<invoke") &&
                                                                xml_text_buffer.chars().filter(|&c| c == '>').count() > 3) {
                                                                #[cfg(feature = "trace")]
                                                                debug!("XML 緩衝區過大或不包含工具調用，發送為普通文本");
                                                                // 發送緩衝的文本
                                                                events.push(Ok(ChatResponse {
                                                                    event: event_type.clone(),
                                                                    data: Some(ChatResponseData::Text {
                                                                        text: xml_text_buffer.clone(),
                                                                    }),
                                                                }));
                                                                // 重置緩衝狀態
                                                                xml_text_buffer.clear();
                                                                xml_detection_active = false;
                                                            } else {
                                                                // 繼續緩衝
                                                                #[cfg(feature = "trace")]
                                                                debug!("繼續緩衝 XML 文本，當前長度: {}", xml_text_buffer.len());
                                                            }
                                                        }
                                                    } else {
                                                        // 沒有檢測到 XML，直接發送文本
                                                        events.push(Ok(ChatResponse {
                                                            event: event_type.clone(),
                                                            data: Some(ChatResponseData::Text {
                                                                text: text.to_string(),
                                                            }),
                                                        }));
                                                    }
                                                }

                                                #[cfg(not(feature = "xml"))]
                                                {
                                                    events.push(Ok(ChatResponse {
                                                        event: event_type.clone(),
                                                        data: Some(ChatResponseData::Text {
                                                            text: text.to_string(),
                                                        }),
                                                    }));
                                                }
                                            }
                                        } else {
                                            #[cfg(feature = "trace")]
                                            debug!("JSON 解析失敗，可能是不完整的數據，等待更多數據");
                                            is_collecting_data = true;
                                        }
                                    }
                                    ChatEventType::File => {
                                        if let Ok(file_data) = serde_json::from_str::<FileData>(data) {
                                            #[cfg(feature = "trace")]
                                            debug!("解析到文件數據: {}", file_data.name);
                                            events.push(Ok(ChatResponse {
                                                event: ChatEventType::File,
                                                data: Some(ChatResponseData::File(file_data)),
                                            }));
                                        } else {
                                            #[cfg(feature = "trace")]
                                            debug!("文件數據 JSON 解析失敗，可能是不完整的數據，等待更多數據");
                                            is_collecting_data = true;
                                        }
                                    }
                                    ChatEventType::Json => {
                                        if let Ok(json) = serde_json::from_str::<Value>(data) {
                                            #[cfg(feature = "trace")]
                                            debug!("解析到 JSON 事件數據");
                                            // 檢查是否有 finish_reason: "tool_calls"，表示工具調用完成
                                            let finish_reason = json
                                                .get("choices")
                                                .and_then(|choices| choices.get(0))
                                                .and_then(|choice| choice.get("finish_reason"))
                                                .and_then(Value::as_str);

                                            if finish_reason == Some("tool_calls") {
                                                #[cfg(feature = "trace")]
                                                debug!("檢測到工具調用完成標誌");
                                                tool_calls_complete = true;
                                            }

                                            // 檢查是否包含 tool_calls delta
                                            let tool_calls_delta = json
                                                .get("choices")
                                                .and_then(|choices| choices.get(0))
                                                .and_then(|choice| choice.get("delta"))
                                                .and_then(|delta| delta.get("tool_calls"));

                                            if let Some(tool_calls_array) = tool_calls_delta {
                                                #[cfg(feature = "trace")]
                                                debug!("檢測到工具調用 delta");
                                                // 處理每個工具調用的 delta
                                                if let Some(tool_calls) = tool_calls_array.as_array() {
                                                    for tool_call_delta in tool_calls {
                                                        let index = tool_call_delta
                                                            .get("index")
                                                            .and_then(Value::as_u64)
                                                            .unwrap_or(0)
                                                            as usize;

                                                        // 確保 accumulated_tool_calls 有足夠的元素
                                                        while accumulated_tool_calls.len() <= index {
                                                            accumulated_tool_calls.push(PartialToolCall::default());
                                                        }

                                                        // 更新 id 和 type
                                                        if let Some(id) = tool_call_delta
                                                            .get("id")
                                                            .and_then(Value::as_str)
                                                        {
                                                            accumulated_tool_calls[index].id = id.to_string();
                                                        }

                                                        if let Some(type_str) = tool_call_delta
                                                            .get("type")
                                                            .and_then(Value::as_str)
                                                        {
                                                            accumulated_tool_calls[index].r#type = type_str.to_string();
                                                        }

                                                        // 更新 function 相關欄位
                                                        if let Some(function) = tool_call_delta.get("function") {
                                                            if let Some(name) = function
                                                                .get("name")
                                                                .and_then(Value::as_str)
                                                            {
                                                                accumulated_tool_calls[index].function_name = name.to_string();
                                                            }

                                                            if let Some(args) = function
                                                                .get("arguments")
                                                                .and_then(Value::as_str)
                                                            {
                                                                accumulated_tool_calls[index].function_arguments.push_str(args);
                                                            }
                                                        }
                                                    }
                                                }
                                            } else if !tool_calls_complete {
                                                // 如果沒有 tool_calls delta 且工具調用尚未完成，
                                                // 則按一般 JSON 處理
                                                events.push(Ok(ChatResponse {
                                                    event: ChatEventType::Json,
                                                    data: Some(ChatResponseData::Text {
                                                        text: data.to_string(),
                                                    }),
                                                }));
                                            }
                                        } else {
                                            #[cfg(feature = "trace")]
                                            debug!("JSON 事件解析失敗，可能是不完整的數據");
                                            is_collecting_data = true;
                                        }
                                    }
                                    ChatEventType::Done => {
                                        #[cfg(feature = "trace")]
                                        debug!("收到完成事件");
                                        // 處理任何剩餘的 XML 緩衝內容
                                        #[cfg(feature = "xml")]
                                        {
                                            if xml_detection_active && !xml_text_buffer.trim().is_empty() {
                                                #[cfg(feature = "trace")]
                                                debug!("處理剩餘的 XML 緩衝內容，長度: {}", xml_text_buffer.len());
                                                let message = ChatMessage {
                                                    role: "assistant".to_string(),
                                                    content: xml_text_buffer.clone(),
                                                    attachments: None,
                                                    content_type: "text/plain".to_string(),
                                                };
                                                if message.contains_xml_tool_calls() {
                                                    let tool_calls = message.extract_xml_tool_calls();
                                                    if !tool_calls.is_empty() {
                                                        #[cfg(feature = "trace")]
                                                        debug!("在完成事件中檢測到 XML 工具調用，數量: {}", tool_calls.len());
                                                        // 發送工具調用事件
                                                        events.push(Ok(ChatResponse {
                                                            event: ChatEventType::Json,
                                                            data: Some(ChatResponseData::ToolCalls(tool_calls)),
                                                        }));
                                                        // 發送清理後的文本（如果有）
                                                        let clean_text = Self::remove_xml_tool_calls(&xml_text_buffer);
                                                        if !clean_text.trim().is_empty() {
                                                            events.push(Ok(ChatResponse {
                                                                event: ChatEventType::Text,
                                                                data: Some(ChatResponseData::Text {
                                                                    text: clean_text,
                                                                }),
                                                            }));
                                                        }
                                                    } else {
                                                        // 發送為普通文本
                                                        events.push(Ok(ChatResponse {
                                                            event: ChatEventType::Text,
                                                            data: Some(ChatResponseData::Text {
                                                                text: xml_text_buffer.clone(),
                                                            }),
                                                        }));
                                                    }
                                                } else {
                                                    // 發送為普通文本
                                                    events.push(Ok(ChatResponse {
                                                        event: ChatEventType::Text,
                                                        data: Some(ChatResponseData::Text {
                                                            text: xml_text_buffer.clone(),
                                                        }),
                                                    }));
                                                }
                                                // 清理緩衝狀態
                                                xml_text_buffer.clear();
                                                xml_detection_active = false;
                                            }
                                        }
                                        events.push(Ok(ChatResponse {
                                            event: ChatEventType::Done,
                                            data: Some(ChatResponseData::Empty),
                                        }));
                                        current_event = None;
                                    }
                                    ChatEventType::Error => {
                                        if let Ok(json) = serde_json::from_str::<Value>(data) {
                                            let text = json
                                                .get("text")
                                                .and_then(Value::as_str)
                                                .unwrap_or("未知錯誤");
                                            let allow_retry = json
                                                .get("allow_retry")
                                                .and_then(Value::as_bool)
                                                .unwrap_or(false);

                                            #[cfg(feature = "trace")]
                                            warn!("收到錯誤事件: {}, 可重試: {}", text, allow_retry);

                                            events.push(Ok(ChatResponse {
                                                event: ChatEventType::Error,
                                                data: Some(ChatResponseData::Error {
                                                    text: text.to_string(),
                                                    allow_retry,
                                                }),
                                            }));
                                        } else {
                                            #[cfg(feature = "trace")]
                                            warn!("無法解析錯誤事件數據: {}", data);
                                        }
                                        current_event = None;
                                    }
                                }
                            } else {
                                #[cfg(feature = "trace")]
                                debug!("收到數據但沒有當前事件類型");
                            }
                        } else if is_collecting_data {
                            // 嘗試解析累積的 JSON
                            #[cfg(feature = "trace")]
                            debug!("嘗試解析未完整的 JSON 數據: {}", line);

                            if let Some(ref event_type) = current_event {
                                match event_type {
                                    ChatEventType::Text | ChatEventType::ReplaceResponse => {
                                        if let Ok(json) = serde_json::from_str::<Value>(&line) {
                                            if let Some(text) = json.get("text").and_then(Value::as_str) {
                                                #[cfg(feature = "trace")]
                                                debug!("成功解析到累積的 JSON 文本，長度: {}", text.len());

                                                events.push(Ok(ChatResponse {
                                                    event: event_type.clone(),
                                                    data: Some(ChatResponseData::Text {
                                                        text: text.to_string(),
                                                    }),
                                                }));
                                                is_collecting_data = false;
                                                current_event = None;
                                            }
                                        }
                                    }
                                    ChatEventType::File => {
                                        if let Ok(file_data) = serde_json::from_str::<FileData>(&line) {
                                            #[cfg(feature = "trace")]
                                            debug!("成功解析到累積的文件數據: {}", file_data.name);

                                            events.push(Ok(ChatResponse {
                                                event: ChatEventType::File,
                                                data: Some(ChatResponseData::File(file_data)),
                                            }));
                                            is_collecting_data = false;
                                            current_event = None;
                                        }
                                    }
                                    ChatEventType::Json => {
                                        if let Ok(json) = serde_json::from_str::<Value>(&line) {
                                            #[cfg(feature = "trace")]
                                            debug!("成功解析到累積的 JSON 事件數據");

                                            // 檢查是否有 finish_reason: "tool_calls"
                                            let finish_reason = json
                                                .get("choices")
                                                .and_then(|choices| choices.get(0))
                                                .and_then(|choice| choice.get("finish_reason"))
                                                .and_then(Value::as_str);

                                            if finish_reason == Some("tool_calls") {
                                                #[cfg(feature = "trace")]
                                                debug!("檢測到工具調用完成標誌");
                                                tool_calls_complete = true;
                                            }

                                            // 檢查是否包含 tool_calls delta
                                            let tool_calls_delta = json
                                                .get("choices")
                                                .and_then(|choices| choices.get(0))
                                                .and_then(|choice| choice.get("delta"))
                                                .and_then(|delta| delta.get("tool_calls"));

                                            if let Some(tool_calls_array) = tool_calls_delta {
                                                #[cfg(feature = "trace")]
                                                debug!("檢測到工具調用 delta");

                                                // 處理每個工具調用的 delta
                                                if let Some(tool_calls) = tool_calls_array.as_array() {
                                                    for tool_call_delta in tool_calls {
                                                        let index = tool_call_delta
                                                            .get("index")
                                                            .and_then(Value::as_u64)
                                                            .unwrap_or(0)
                                                            as usize;

                                                        // 確保 accumulated_tool_calls 有足夠的元素
                                                        while accumulated_tool_calls.len() <= index {
                                                            accumulated_tool_calls.push(PartialToolCall::default());
                                                        }

                                                        // 更新 id 和 type
                                                        if let Some(id) = tool_call_delta
                                                            .get("id")
                                                            .and_then(Value::as_str)
                                                        {
                                                            accumulated_tool_calls[index].id = id.to_string();
                                                        }

                                                        if let Some(type_str) = tool_call_delta
                                                            .get("type")
                                                            .and_then(Value::as_str)
                                                        {
                                                            accumulated_tool_calls[index].r#type = type_str.to_string();
                                                        }

                                                        // 更新 function 相關欄位
                                                        if let Some(function) = tool_call_delta.get("function") {
                                                            if let Some(name) = function
                                                                .get("name")
                                                                .and_then(Value::as_str)
                                                            {
                                                                accumulated_tool_calls[index].function_name = name.to_string();
                                                            }

                                                            if let Some(args) = function
                                                                .get("arguments")
                                                                .and_then(Value::as_str)
                                                            {
                                                                accumulated_tool_calls[index].function_arguments.push_str(args);
                                                            }
                                                        }
                                                    }
                                                }

                                                // 如果工具調用完成，則創建並發送 ChatResponse
                                                if tool_calls_complete && !accumulated_tool_calls.is_empty() {
                                                    let complete_tool_calls = accumulated_tool_calls
                                                        .iter()
                                                        .filter(|tc| {
                                                            !tc.id.is_empty() && !tc.function_name.is_empty()
                                                        })
                                                        .map(|tc| ChatToolCall {
                                                            id: tc.id.clone(),
                                                            r#type: tc.r#type.clone(),
                                                            function: FunctionCall {
                                                                name: tc.function_name.clone(),
                                                                arguments: tc.function_arguments.clone(),
                                                            },
                                                        })
                                                        .collect::<Vec<ChatToolCall>>();

                                                    if !complete_tool_calls.is_empty() {
                                                        #[cfg(feature = "trace")]
                                                        debug!("發送完整的工具調用，數量: {}", complete_tool_calls.len());

                                                        events.push(Ok(ChatResponse {
                                                            event: ChatEventType::Json,
                                                            data: Some(ChatResponseData::ToolCalls(complete_tool_calls)),
                                                        }));

                                                        // 重置累積狀態
                                                        accumulated_tool_calls.clear();
                                                        tool_calls_complete = false;
                                                    }
                                                }
                                            } else {
                                                // 如果沒有 tool_calls delta，則按一般 JSON 處理
                                                events.push(Ok(ChatResponse {
                                                    event: ChatEventType::Json,
                                                    data: Some(ChatResponseData::Text {
                                                        text: line.to_string(),
                                                    }),
                                                }));
                                            }

                                            is_collecting_data = false;
                                            current_event = None;
                                        }
                                    }
                                    ChatEventType::Done | ChatEventType::Error => {
                                        // 這些事件類型不應該有累積的數據
                                        is_collecting_data = false;
                                    }
                                }
                            }
                        }
                    }

                    // 在處理完 chunk 中的所有行之後，檢查是否需要發送最終的 tool_calls 事件
                    if tool_calls_complete && !accumulated_tool_calls.is_empty() {
                        let complete_tool_calls = accumulated_tool_calls
                            .iter()
                            .filter(|tc| !tc.id.is_empty() && !tc.function_name.is_empty())
                            .map(|tc| ChatToolCall {
                                id: tc.id.clone(),
                                r#type: tc.r#type.clone(),
                                function: FunctionCall {
                                    name: tc.function_name.clone(),
                                    arguments: tc.function_arguments.clone(),
                                },
                            })
                            .collect::<Vec<ChatToolCall>>();

                        if !complete_tool_calls.is_empty() {
                            #[cfg(feature = "trace")]
                            debug!("發送最終的完整工具調用，數量: {}", complete_tool_calls.len());

                            events.push(Ok(ChatResponse {
                                event: ChatEventType::Json,
                                data: Some(ChatResponseData::ToolCalls(complete_tool_calls)),
                            }));

                            // 重置狀態
                            accumulated_tool_calls.clear();
                            tool_calls_complete = false;
                        }
                    }

                    events
                })
            })
            .flat_map(|result| {
                futures_util::stream::iter(match result {
                    Ok(events) => events,
                    Err(e) => {
                        #[cfg(feature = "trace")]
                        warn!("串流處理錯誤: {}", e);
                        vec![Err(e)]
                    }
                })
            });

        Ok(Box::pin(stream))
    }

    pub async fn send_tool_results(
        &self,
        original_request: ChatRequest,
        tool_calls: Vec<ChatToolCall>,
        tool_results: Vec<ChatToolResult>,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<ChatResponse, PoeError>> + Send>>, PoeError> {
        #[cfg(feature = "trace")]
        debug!("發送工具調用結果，bot_name: {}", self.bot_name);

        // 創建包含工具結果的新請求
        let mut request = original_request;

        // 當啟用 xml feature 時，將工具結果以 XML 格式附加到訊息末尾
        #[cfg(feature = "xml")]
        {
            #[cfg(feature = "trace")]
            debug!("檢測到 xml feature 啟用，將工具結果轉換為 XML 格式並附加到訊息末尾");

            // 先設置工具調用和結果，以便 XML 轉換方法可以訪問
            request.tool_calls = Some(tool_calls);
            request.tool_results = Some(tool_results);

            // 將工具結果轉換為 XML 格式並附加到訊息末尾
            request.append_tool_results_as_xml();

            // 清除原始的工具調用和結果，因為已經轉換為 XML 格式
            request.tool_calls = None;
            request.tool_results = None;

            #[cfg(feature = "trace")]
            debug!(
                "🔧 工具結果 XML 轉換完成，檢查訊息內容: {}",
                request
                    .query
                    .iter()
                    .map(|msg| format!("角色: {}, 內容長度: {}", msg.role, msg.content.len()))
                    .collect::<Vec<_>>()
                    .join("; ")
            );
        }

        // 當未啟用 xml feature 時，使用原有的 JSON API 方式
        #[cfg(not(feature = "xml"))]
        {
            request.tool_calls = Some(tool_calls);
            request.tool_results = Some(tool_results);
        }

        #[cfg(feature = "trace")]
        debug!(
            "發送工具結果請求結構: {}",
            serde_json::to_string_pretty(&request).unwrap_or_else(|_| "無法序列化請求".to_string())
        );

        // 發送請求並處理響應（stream_request 會自動處理 XML feature）
        self.stream_request(request).await
    }

    /// 上傳本地檔案
    pub async fn upload_local_file(
        &self,
        file_path: &str,
        mime_type: Option<&str>,
    ) -> Result<FileUploadResponse, PoeError> {
        #[cfg(feature = "trace")]
        debug!(
            "開始上傳本地檔案: {} | MIME 類型: {:?}",
            file_path, mime_type
        );
        // 檢查檔案是否存在
        let path = Path::new(file_path);
        if !path.exists() {
            #[cfg(feature = "trace")]
            warn!("檔案不存在: {}", file_path);
            return Err(PoeError::FileNotFound(file_path.to_string()));
        }

        // 簡化 MIME 類型處理：如果有提供 mime_type 就使用，否則使用預設值
        let content_type = mime_type.unwrap_or("application/octet-stream").to_string();

        #[cfg(feature = "trace")]
        debug!("使用 MIME 類型: {}", content_type);

        // 建立 multipart 表單
        let file = tokio::fs::File::open(path).await.map_err(|e| {
            #[cfg(feature = "trace")]
            warn!("無法開啟檔案: {}", e);
            PoeError::FileReadError(e)
        })?;

        let file_part =
            reqwest::multipart::Part::stream(reqwest::Body::wrap_stream(ReaderStream::new(file)))
                .file_name(
                    path.file_name()
                        .and_then(|name| name.to_str())
                        .unwrap_or("file")
                        .to_string(),
                )
                .mime_str(&content_type)
                .map_err(|e| {
                    #[cfg(feature = "trace")]
                    warn!("設置 MIME 類型失敗: {}", e);
                    PoeError::FileUploadFailed(format!("設置 MIME 類型失敗: {}", e))
                })?;

        let form = reqwest::multipart::Form::new().part("file", file_part);

        // 發送請求
        self.send_upload_request(form).await
    }

    /// 上傳遠端檔案 (通過URL)
    pub async fn upload_remote_file(
        &self,
        download_url: &str,
    ) -> Result<FileUploadResponse, PoeError> {
        #[cfg(feature = "trace")]
        debug!("開始上傳遠端檔案: {}", download_url);

        // 檢查URL格式
        url::Url::parse(download_url)?;

        // 建立 multipart 表單
        let form = reqwest::multipart::Form::new().text("download_url", download_url.to_string());

        // 發送請求
        self.send_upload_request(form).await
    }

    /// 批量上傳檔案 (接受混合的本地和遠端檔案)
    pub async fn upload_files_batch(
        &self,
        files: Vec<FileUploadRequest>,
    ) -> Result<Vec<FileUploadResponse>, PoeError> {
        #[cfg(feature = "trace")]
        debug!("開始批量上傳檔案，數量: {}", files.len());

        if files.is_empty() {
            return Ok(Vec::new());
        }

        // 為每個檔案創建上傳任務
        let mut upload_tasks = Vec::with_capacity(files.len());

        for file_request in files {
            let task = match file_request {
                FileUploadRequest::LocalFile { file, mime_type } => {
                    let client = self.clone();
                    let file_path = file.clone();
                    tokio::spawn(async move {
                        client
                            .upload_local_file(&file_path, mime_type.as_deref())
                            .await
                    })
                }
                FileUploadRequest::RemoteFile { download_url } => {
                    let client = self.clone();
                    let url = download_url.clone();
                    tokio::spawn(async move { client.upload_remote_file(&url).await })
                }
            };
            upload_tasks.push(task);
        }

        // 等待所有上傳任務完成
        let results = join_all(upload_tasks).await;

        // 收集結果
        let mut upload_responses = Vec::with_capacity(results.len());

        for task_result in results.into_iter() {
            match task_result {
                Ok(upload_result) => match upload_result {
                    Ok(response) => {
                        #[cfg(feature = "trace")]
                        debug!("檔案上傳成功: {}", response.attachment_url);
                        upload_responses.push(response);
                    }
                    Err(e) => {
                        #[cfg(feature = "trace")]
                        warn!("檔案上傳失敗: {}", e);
                        return Err(e);
                    }
                },
                Err(e) => {
                    #[cfg(feature = "trace")]
                    warn!("檔案上傳任務失敗: {}", e);
                    return Err(PoeError::FileUploadFailed(format!("上傳任務失敗: {}", e)));
                }
            }
        }

        #[cfg(feature = "trace")]
        debug!("批量上傳全部成功，共 {} 個檔案", upload_responses.len());

        Ok(upload_responses)
    }

    /// 發送檔案上傳請求 (內部方法)
    async fn send_upload_request(
        &self,
        form: reqwest::multipart::Form,
    ) -> Result<FileUploadResponse, PoeError> {
        #[cfg(feature = "trace")]
        debug!("發送檔案上傳請求至 {}", self.poe_file_upload_url);

        let response = self
            .client
            .post(&self.poe_file_upload_url)
            .header("Authorization", format!("Bearer {}", self.access_key))
            .multipart(form)
            .send()
            .await
            .map_err(|e| {
                #[cfg(feature = "trace")]
                warn!("檔案上傳請求失敗: {}", e);
                PoeError::RequestFailed(e)
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response
                .text()
                .await
                .unwrap_or_else(|_| "無法讀取回應內容".to_string());

            #[cfg(feature = "trace")]
            warn!("檔案上傳API回應錯誤 - 狀態碼: {}, 內容: {}", status, text);

            return Err(PoeError::FileUploadFailed(format!(
                "上傳失敗 - 狀態碼: {}, 內容: {}",
                status, text
            )));
        }

        #[cfg(feature = "trace")]
        debug!("成功接收到檔案上傳回應");

        let response_text = response.text().await.map_err(|e| {
            #[cfg(feature = "trace")]
            warn!("讀取檔案上傳回應內容失敗: {}", e);
            PoeError::RequestFailed(e)
        })?;

        #[cfg(feature = "trace")]
        debug!("檔案上傳回應內容: {}", response_text);

        let upload_response: FileUploadResponse =
            serde_json::from_str(&response_text).map_err(|e| {
                #[cfg(feature = "trace")]
                warn!("解析檔案上傳回應失敗: {}", e);
                PoeError::JsonParseFailed(e)
            })?;

        #[cfg(feature = "trace")]
        debug!("檔案上傳成功，附件URL: {}", upload_response.attachment_url);

        Ok(upload_response)
    }

    /// 獲取 v1/models API 的模型列表 (需要 access_key)
    pub async fn get_v1_model_list(&self) -> Result<ModelResponse, PoeError> {
        #[cfg(feature = "trace")]
        debug!("開始獲取 v1/models 模型列表");

        let url = format!("{}/v1/models", self.poe_base_url);
        #[cfg(feature = "trace")]
        debug!("發送 v1/models 請求至 URL: {}", url);

        let response = self
            .client
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.access_key))
            .header("Content-Type", "application/json")
            .send()
            .await
            .map_err(|e| {
                #[cfg(feature = "trace")]
                warn!("發送 v1/models 請求失敗: {}", e);
                PoeError::RequestFailed(e)
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response
                .text()
                .await
                .unwrap_or_else(|_| "無法讀取回應內容".to_string());

            #[cfg(feature = "trace")]
            warn!(
                "v1/models API 回應錯誤 - 狀態碼: {}, 內容: {}",
                status, text
            );

            return Err(PoeError::BotError(format!(
                "v1/models API 回應錯誤 - 狀態碼: {}, 內容: {}",
                status, text
            )));
        }

        #[cfg(feature = "trace")]
        debug!("成功接收到 v1/models 回應");

        let response_text = response.text().await.map_err(|e| {
            #[cfg(feature = "trace")]
            warn!("讀取 v1/models 回應內容失敗: {}", e);
            PoeError::RequestFailed(e)
        })?;

        #[cfg(feature = "trace")]
        debug!("v1/models 回應內容: {}", response_text);

        let json_data: Value = serde_json::from_str(&response_text).map_err(|e| {
            #[cfg(feature = "trace")]
            warn!("解析 v1/models 回應失敗: {}", e);
            PoeError::JsonParseFailed(e)
        })?;

        let mut model_list = Vec::new();

        if let Some(data_array) = json_data.get("data").and_then(Value::as_array) {
            #[cfg(feature = "trace")]
            debug!("找到 {} 個模型", data_array.len());

            for model_data in data_array {
                if let (Some(id), Some(object), Some(created), Some(owned_by)) = (
                    model_data.get("id").and_then(Value::as_str),
                    model_data.get("object").and_then(Value::as_str),
                    model_data.get("created").and_then(Value::as_i64),
                    model_data.get("owned_by").and_then(Value::as_str),
                ) {
                    model_list.push(ModelInfo {
                        id: id.to_string(),
                        object: object.to_string(),
                        created,
                        owned_by: owned_by.to_string(),
                    });
                }
            }
        } else {
            #[cfg(feature = "trace")]
            warn!("無法從 v1/models 回應中取得模型列表");
            return Err(PoeError::BotError(
                "無法從 v1/models 回應中取得模型列表".to_string(),
            ));
        }

        if model_list.is_empty() {
            #[cfg(feature = "trace")]
            warn!("取得的模型列表為空");
            return Err(PoeError::BotError("取得的模型列表為空".to_string()));
        }

        #[cfg(feature = "trace")]
        debug!("成功解析 {} 個模型", model_list.len());

        Ok(ModelResponse { data: model_list })
    }

    /// 從文本中移除 XML 工具調用部分
    #[cfg(feature = "xml")]
    pub fn remove_xml_tool_calls(text: &str) -> String {
        // 創建一個臨時的 ChatMessage 來檢測工具調用
        let message = ChatMessage {
            role: "assistant".to_string(),
            content: text.to_string(),
            attachments: None,
            content_type: "text/plain".to_string(),
        };

        // 如果沒有檢測到工具調用，直接返回原文本
        if !message.contains_xml_tool_calls() {
            return text.to_string();
        }

        // 提取工具調用以了解需要移除哪些部分
        let tool_calls = message.extract_xml_tool_calls();
        if tool_calls.is_empty() {
            return text.to_string();
        }

        let mut result = text.to_string();

        // 移除 <tool_call>...</tool_call> 標籤
        while let Some(start) = result.find("<tool_call>") {
            if let Some(end) = result[start..].find("</tool_call>") {
                let end_pos = start + end + "</tool_call>".len();
                result.replace_range(start..end_pos, "");
            } else {
                break;
            }
        }

        // 根據檢測到的工具調用移除對應的工具標籤
        for tool_call in &tool_calls {
            let tool_name = &tool_call.function.name;
            let start_pattern = format!("<{}>", tool_name);
            let end_pattern = format!("</{}>", tool_name);

            while let Some(start) = result.find(&start_pattern) {
                if let Some(end) = result[start..].find(&end_pattern) {
                    let end_pos = start + end + end_pattern.len();
                    result.replace_range(start..end_pos, "");
                } else {
                    break;
                }
            }
        }

        // 移除 <invoke> 標籤（如果存在）
        while let Some(start) = result.find("<invoke") {
            if let Some(end) = result[start..].find("</invoke>") {
                let end_pos = start + end + "</invoke>".len();
                result.replace_range(start..end_pos, "");
            } else {
                break;
            }
        }

        // 清理多餘的空行
        result
            .lines()
            .filter(|line| !line.trim().is_empty())
            .collect::<Vec<_>>()
            .join("\n")
    }
}

pub async fn get_model_list(language_code: Option<&str>) -> Result<ModelResponse, PoeError> {
    #[cfg(feature = "trace")]
    debug!("開始獲取模型列表，語言代碼: {:?}", language_code);

    let client = Client::builder()
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
        .build()
        .map_err(|e| {
            #[cfg(feature = "trace")]
            warn!("建立 HTTP 客戶端失敗: {}", e);
            PoeError::BotError(e.to_string())
        })?;

    let payload = serde_json::json!({
        "queryName": "ExploreBotsListPaginationQuery",
        "variables": {
            "categoryName": "defaultCategory",
            "count": 150
        },
        "extensions": {
            "hash": POE_GQL_MODEL_HASH
        }
    });

    #[cfg(feature = "trace")]
    debug!("準備 GraphQL 請求載荷，使用 hash: {}", POE_GQL_MODEL_HASH);

    let mut headers = HeaderMap::new();
    headers.insert("Content-Type", HeaderValue::from_static("application/json"));
    headers.insert("Accept", HeaderValue::from_static("*/*"));
    headers.insert(
        "Accept-Language",
        HeaderValue::from_static("zh-TW,zh;q=0.9,en-US;q=0.8,en;q=0.7"),
    );
    headers.insert("Origin", HeaderValue::from_static("https://poe.com"));
    headers.insert("Referer", HeaderValue::from_static("https://poe.com"));
    headers.insert("Sec-Fetch-Dest", HeaderValue::from_static("empty"));
    headers.insert("Sec-Fetch-Mode", HeaderValue::from_static("cors"));
    headers.insert("Sec-Fetch-Site", HeaderValue::from_static("same-origin"));
    headers.insert(
        "poe-revision",
        HeaderValue::from_static(POE_GQL_MODEL_REVISION),
    );
    headers.insert("poegraphql", HeaderValue::from_static("1"));

    if let Some(code) = language_code {
        let cookie_value = format!("Poe-Language-Code={}; p-b=1", code);
        #[cfg(feature = "trace")]
        debug!("設置語言 Cookie: {}", cookie_value);

        headers.insert(
            COOKIE,
            HeaderValue::from_str(&cookie_value).map_err(|e| {
                #[cfg(feature = "trace")]
                warn!("設置 Cookie 失敗: {}", e);
                PoeError::BotError(e.to_string())
            })?,
        );
    }

    #[cfg(feature = "trace")]
    debug!("發送 GraphQL 請求至 {}", POE_GQL_URL);

    let response = client
        .post(POE_GQL_URL)
        .headers(headers)
        .json(&payload)
        .send()
        .await
        .map_err(|e| {
            #[cfg(feature = "trace")]
            warn!("發送 GraphQL 請求失敗: {}", e);
            PoeError::RequestFailed(e)
        })?;

    if !response.status().is_success() {
        let status = response.status();
        let text = response
            .text()
            .await
            .unwrap_or_else(|_| "無法讀取回應內容".to_string());

        #[cfg(feature = "trace")]
        warn!("GraphQL API 回應錯誤 - 狀態碼: {}, 內容: {}", status, text);

        return Err(PoeError::BotError(format!(
            "API 回應錯誤 - 狀態碼: {}, 內容: {}",
            status, text
        )));
    }

    #[cfg(feature = "trace")]
    debug!("成功接收到 GraphQL 回應");

    let json_value = response.text().await.map_err(|e| {
        #[cfg(feature = "trace")]
        warn!("讀取 GraphQL 回應內容失敗: {}", e);
        PoeError::RequestFailed(e)
    })?;

    let data: Value = serde_json::from_str(&json_value).map_err(|e| {
        #[cfg(feature = "trace")]
        warn!("解析 GraphQL 回應 JSON 失敗: {}", e);
        PoeError::JsonParseFailed(e)
    })?;

    let mut model_list = Vec::with_capacity(150);

    if let Some(edges) = data["data"]["exploreBotsConnection"]["edges"].as_array() {
        #[cfg(feature = "trace")]
        debug!("找到 {} 個模型節點", edges.len());

        for edge in edges {
            if let Some(handle) = edge["node"]["handle"].as_str() {
                #[cfg(feature = "trace")]
                debug!("解析模型 ID: {}", handle);

                model_list.push(ModelInfo {
                    id: handle.to_string(),
                    object: "model".to_string(),
                    created: 0,
                    owned_by: "poe".to_string(),
                });
            } else {
                #[cfg(feature = "trace")]
                debug!("模型節點中找不到 handle 欄位");
            }
        }
    } else {
        #[cfg(feature = "trace")]
        warn!("無法從回應中取得模型列表節點");
        return Err(PoeError::BotError("無法從回應中取得模型列表".to_string()));
    }

    if model_list.is_empty() {
        #[cfg(feature = "trace")]
        warn!("取得的模型列表為空");
        return Err(PoeError::BotError("取得的模型列表為空".to_string()));
    }

    #[cfg(feature = "trace")]
    debug!("成功解析 {} 個模型", model_list.len());

    Ok(ModelResponse { data: model_list })
}
