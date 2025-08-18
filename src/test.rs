use crate::types::{
    ChatEventType, ChatMessage, ChatRequest, ChatResponseData, ChatTool, ChatToolCall,
    FunctionDefinition, FunctionParameters,
};
use crate::{Attachment, FileUploadRequest, PoeClient, get_model_list};
use dotenvy::dotenv;
use futures_util::StreamExt;
use serde_json::json;
use std::env;
use std::sync::Once;
use tracing::{debug, warn};

// 初始化日誌，確保只執行一次
static INIT: Once = Once::new();

fn setup() {
    // 初始化日誌
    INIT.call_once(|| {
        let _ = env_logger::builder().is_test(true).try_init();
    });
    // 載入環境變數
    dotenv().ok();
    debug!("測試環境設定完成");
}

fn get_access_key() -> String {
    match env::var("POE_ACCESS_KEY") {
        Ok(key) => {
            debug!("成功讀取 POE_ACCESS_KEY 環境變數");
            key
        }
        Err(_) => {
            warn!("無法讀取 POE_ACCESS_KEY 環境變數");
            panic!("需要在 .env 檔案中設置 POE_ACCESS_KEY");
        }
    }
}

#[test_log::test(tokio::test)]
async fn test_stream_request() {
    setup();
    let access_key = get_access_key();
    debug!("建立 PoeClient 測試實例");
    let client = PoeClient::new(
        "Claude-3.7-Sonnet",
        &access_key,
        "https://api.poe.com",
        "https://www.quora.com/poe_api/file_upload_3RD_PARTY_POST",
    );

    let request = ChatRequest {
        version: "1.1".to_string(),
        r#type: "query".to_string(),
        query: vec![ChatMessage {
            role: "user".to_string(),
            content: "Hello".to_string(),
            content_type: "text/markdown".to_string(),
            attachments: None,
        }],
        temperature: None,
        user_id: String::new(),
        conversation_id: String::new(),
        message_id: String::new(),
        tools: None,
        tool_calls: None,
        tool_results: None,
        logit_bias: None,
        stop_sequences: None,
    };

    debug!("發送串流請求");
    let result = client.stream_request(request).await;

    match &result {
        Ok(_) => debug!("串流請求成功"),
        Err(e) => warn!("串流請求失敗: {}", e),
    }

    assert!(result.is_ok(), "建立串流請求應該成功");

    if let Ok(mut stream) = result {
        let mut received_response = false;
        debug!("開始處理回應串流");

        while let Some(response) = stream.next().await {
            match response {
                Ok(event) => {
                    received_response = true;
                    debug!("收到事件: {:?}", event);
                }
                Err(e) => {
                    warn!("串流處理發生錯誤: {}", e);
                    panic!("串流處理發生錯誤: {}", e);
                }
            }
        }

        assert!(received_response, "應該收到至少一個回應");
        debug!("串流請求測試完成");
    }
}

#[test_log::test(tokio::test)]
async fn test_get_model_list() {
    setup();
    debug!("開始測試獲取模型列表");
    let result = get_model_list(Some("zh-Hant")).await;

    match &result {
        Ok(models) => debug!("成功獲取模型列表，共 {} 個模型", models.data.len()),
        Err(e) => warn!("獲取模型列表失敗: {}", e),
    }

    match result {
        Ok(models) => {
            assert!(!models.data.is_empty(), "模型列表不應為空");
            debug!("成功獲取 {} 個模型", models.data.len());
            // 驗證第一個模型的基本資訊
            if let Some(first_model) = models.data.first() {
                assert!(!first_model.id.is_empty(), "模型 ID 不應為空");
                assert_eq!(first_model.object, "model", "模型類型應為 'model'");
                assert_eq!(first_model.owned_by, "poe", "模型擁有者應為 'poe'");
                debug!("第一個模型資訊：");
                debug!("ID: {}", first_model.id);
                debug!("類型: {}", first_model.object);
                debug!("擁有者: {}", first_model.owned_by);
            }
        }
        Err(e) => {
            warn!("獲取模型列表失敗: {}", e);
            panic!("獲取模型列表失敗: {}", e);
        }
    }

    debug!("獲取模型列表測試完成");
}

#[test_log::test(tokio::test)]
async fn test_stream_content_verification() {
    setup();
    let access_key = get_access_key();
    debug!("建立 PoeClient 測試實例");
    let client = PoeClient::new(
        "Claude-3.7-Sonnet",
        &access_key,
        "https://api.poe.com",
        "https://www.quora.com/poe_api/file_upload_3RD_PARTY_POST",
    );

    let request = ChatRequest {
        version: "1.1".to_string(),
        r#type: "query".to_string(),
        query: vec![ChatMessage {
            role: "user".to_string(),
            content: "Say 'hello' only".to_string(),
            content_type: "text/markdown".to_string(),
            attachments: None,
        }],
        temperature: None,
        user_id: String::new(),
        conversation_id: String::new(),
        message_id: String::new(),
        tools: None,
        tool_calls: None,
        tool_results: None,
        logit_bias: None,
        stop_sequences: None,
    };

    debug!("發送串流請求以驗證內容");
    let result = client.stream_request(request).await;

    match &result {
        Ok(_) => debug!("串流請求成功"),
        Err(e) => warn!("串流請求失敗: {}", e),
    }

    assert!(result.is_ok(), "建立串流請求應該成功");

    if let Ok(mut stream) = result {
        let mut received_text_event = false;
        debug!("開始處理回應串流以驗證內容");

        while let Some(response) = stream.next().await {
            match response {
                Ok(event_response) => {
                    debug!("收到事件回應: {:?}", event_response);
                    match event_response.event {
                        ChatEventType::Text => {
                            received_text_event = true;
                            if let Some(ChatResponseData::Text { text }) = event_response.data {
                                debug!("收到 Text 事件，內容: '{}'", text);
                            }
                        }
                        ChatEventType::Error => {
                            if let Some(ChatResponseData::Error { text, allow_retry }) =
                                event_response.data
                            {
                                warn!(
                                    "收到 Error 事件: 錯誤訊息: {}, 可重試: {}",
                                    text, allow_retry
                                );
                                panic!("串流處理收到 Error 事件: {}", text);
                            }
                        }
                        _ => {
                            debug!("收到其他類型的事件: {:?}", event_response.event);
                        }
                    }
                }
                Err(e) => {
                    warn!("串流處理發生錯誤: {}", e);
                    panic!("串流處理發生錯誤: {}", e);
                }
            }
        }

        assert!(received_text_event, "應該收到至少一個 Event::Text 事件");
        debug!("串流內容驗證測試完成");
    }
}

#[test_log::test(tokio::test)]
async fn test_stream_tool_content_verification() {
    setup();
    let access_key = get_access_key();
    debug!("建立 PoeClient 測試實例進行工具內容測試");
    let client = PoeClient::new(
        "GPT-4o-Mini",
        &access_key,
        "https://api.poe.com",
        "https://www.quora.com/poe_api/file_upload_3RD_PARTY_POST",
    );

    // 創建帶有工具定義的請求
    let request = ChatRequest {
        version: "1.1".to_string(),
        r#type: "query".to_string(),
        query: vec![ChatMessage {
            role: "user".to_string(),
            content: "What's the current weather in Taipei? Use the weather tool.".to_string(),
            content_type: "text/markdown".to_string(),
            attachments: None,
        }],
        temperature: None,
        user_id: String::new(),
        conversation_id: String::new(),
        message_id: String::new(),
        tools: Some(vec![ChatTool {
            r#type: "function".to_string(),
            function: FunctionDefinition {
                name: "get_weather".to_string(),
                description: Some("Get weather information for a location".to_string()),
                parameters: Some(FunctionParameters {
                    r#type: "object".to_string(),
                    properties: json!({
                        "location": {
                            "type": "string",
                            "description": "The city and state, e.g. San Francisco, CA"
                        },
                        "unit": {
                            "type": "string",
                            "enum": ["celsius", "fahrenheit"],
                            "description": "The unit of temperature"
                        }
                    }),
                    required: vec!["location".to_string()],
                }),
            },
        }]),
        tool_calls: None,
        tool_results: None,
        logit_bias: None,
        stop_sequences: None,
    };

    debug!("發送帶有工具定義的串流請求");
    let result = client.stream_request(request).await;

    match &result {
        Ok(_) => debug!("工具串流請求成功"),
        Err(e) => warn!("工具串流請求失敗: {}", e),
    }

    assert!(result.is_ok(), "建立工具串流請求應該成功");

    if let Ok(mut stream) = result {
        let mut received_tool_call = false;
        debug!("開始處理工具回應串流");

        while let Some(response) = stream.next().await {
            match response {
                Ok(event_response) => {
                    debug!("收到工具相關事件: {:?}", event_response.event);

                    match event_response.event {
                        ChatEventType::Json => {
                            // 檢查是否有 tool_calls
                            if let Some(ChatResponseData::ToolCalls(tool_calls)) =
                                event_response.data
                            {
                                received_tool_call = true;
                                debug!("收到 ToolCalls 事件，工具調用數量: {}", tool_calls.len());

                                // 驗證工具調用的內容
                                for tool_call in tool_calls {
                                    assert_eq!(
                                        tool_call.r#type, "function",
                                        "工具調用類型應為 function"
                                    );
                                    assert!(!tool_call.id.is_empty(), "工具調用 ID 不應為空");
                                    assert_eq!(
                                        tool_call.function.name, "get_weather",
                                        "工具調用函數名應為 get_weather"
                                    );
                                    debug!("工具調用參數: {}", tool_call.function.arguments);
                                }

                                // 因為我們已經確認收到工具調用，可以選擇退出循環
                                break;
                            } else {
                                debug!("收到 Json 事件，但不包含 tool_calls，可能是增量更新");
                            }
                        }
                        ChatEventType::Error => {
                            if let Some(ChatResponseData::Error { text, allow_retry }) =
                                event_response.data
                            {
                                warn!(
                                    "收到 Error 事件: 錯誤訊息: {}, 可重試: {}",
                                    text, allow_retry
                                );
                                panic!("工具串流處理收到 Error 事件: {}", text);
                            }
                        }
                        _ => {
                            debug!("收到其他類型的事件: {:?}", event_response.event);
                        }
                    }
                }
                Err(e) => {
                    warn!("工具串流處理發生錯誤: {}", e);
                    panic!("工具串流處理發生錯誤: {}", e);
                }
            }
        }

        // 注意：取決於模型的回應，工具調用不一定總會發生
        // 因此這裡不使用嚴格斷言，而是記錄結果
        if received_tool_call {
            debug!("成功收到工具調用事件");
        } else {
            debug!("沒有收到工具調用事件，這可能是正常的，取決於模型回應");
        }

        debug!("工具相關串流測試完成");
    }
}

#[test_log::test(tokio::test)]
async fn test_tool_calls_parsing() {
    setup();
    debug!("開始測試工具調用解析");

    // 模擬的工具調用 JSON 數據
    let tool_calls_json = json!({
        "tool_calls": [
            {
                "id": "call_123456",
                "type": "function",
                "function": {
                    "name": "get_weather",
                    "arguments": "{\"location\":\"Taipei\",\"unit\":\"celsius\"}"
                }
            }
        ]
    });

    // 解析工具調用
    let tool_calls_value = tool_calls_json.get("tool_calls").unwrap();
    let tool_calls: Vec<ChatToolCall> = serde_json::from_value(tool_calls_value.clone()).unwrap();

    // 驗證解析結果
    assert_eq!(tool_calls.len(), 1, "應該解析出一個工具調用");
    assert_eq!(tool_calls[0].id, "call_123456", "工具調用 ID 應該匹配");
    assert_eq!(
        tool_calls[0].r#type, "function",
        "工具調用類型應該是 function"
    );
    assert_eq!(
        tool_calls[0].function.name, "get_weather",
        "工具調用函數名應該是 get_weather"
    );
    assert_eq!(
        tool_calls[0].function.arguments, "{\"location\":\"Taipei\",\"unit\":\"celsius\"}",
        "工具調用參數應該匹配"
    );

    debug!("工具調用解析測試完成");
}

#[test_log::test(tokio::test)]
async fn test_tool_call_parse_error() {
    setup();
    debug!("開始測試工具調用解析錯誤處理");

    // 模擬的格式錯誤的工具調用 JSON 數據
    let invalid_tool_calls_json = json!({
        "tool_calls": [
            {
                "id": "call_123456",
                "type": "function",
                "function": {
                    "name": "get_weather",
                    // 缺少 arguments 字段，這將導致解析錯誤
                }
            }
        ]
    });

    // 嘗試解析無效的工具調用
    let tool_calls_value = invalid_tool_calls_json.get("tool_calls").unwrap();
    let parse_result: Result<Vec<ChatToolCall>, _> =
        serde_json::from_value(tool_calls_value.clone());

    // 驗證解析結果應該是錯誤
    assert!(parse_result.is_err(), "解析無效的工具調用應該失敗");

    // 驗證錯誤類型
    let error = parse_result.unwrap_err();
    debug!("解析錯誤: {}", error);
    assert!(
        error.to_string().contains("missing field"),
        "錯誤消息應該指示缺少字段"
    );

    debug!("工具調用解析錯誤處理測試完成");
}

#[test_log::test(tokio::test)]
async fn test_file_upload() {
    setup();
    let access_key = get_access_key();
    debug!("建立 PoeClient 測試實例，用於檔案上傳測試");
    let client = PoeClient::new(
        "Claude-3.7-Sonnet",
        &access_key,
        "https://api.poe.com",
        "https://www.quora.com/poe_api/file_upload_3RD_PARTY_POST",
    );
    // 創建一個臨時文件用於測試
    use std::fs::File;
    use std::io::Write;
    use tempfile::tempdir;
    let temp_dir = tempdir().expect("無法創建臨時目錄");
    let file_path = temp_dir.path().join("test_upload.txt");
    let file_path_str = file_path.to_str().unwrap().to_string();
    debug!("創建臨時測試文件: {}", file_path_str);
    {
        let mut file = File::create(&file_path).expect("無法創建臨時文件");
        writeln!(file, "這是一個測試上傳文件的內容").expect("無法寫入臨時文件");
    }
    // 測試本地文件上傳
    debug!("開始測試本地文件上傳");
    let upload_result = client.upload_local_file(&file_path_str, None).await;
    match &upload_result {
        Ok(response) => {
            debug!("文件上傳成功，附件URL: {}", response.attachment_url);
            debug!("文件MIME類型: {}", response.mime_type.clone().unwrap());
            debug!("文件大小: {} 字節", response.size.unwrap());
        }
        Err(e) => warn!("文件上傳失敗: {}", e),
    }
    assert!(upload_result.is_ok(), "本地文件上傳應該成功");
    if let Ok(response) = upload_result {
        assert!(!response.attachment_url.is_empty(), "附件URL不應為空");
        assert_eq!(
            response.mime_type.unwrap(),
            "text/plain",
            "MIME類型應為text/plain"
        );
        assert!(response.size.unwrap() > 0, "文件大小應大於0");
    }
    // 測試批量上傳
    debug!("開始測試批量上傳");
    let batch_upload_requests = vec![
        FileUploadRequest::LocalFile {
            file: file_path_str.clone(),
            mime_type: None,
        },
        // 可以添加遠程文件測試，但需要有效URL
        // FileUploadRequest::RemoteFile { download_url: "https://example.com/sample.txt".to_string() },
    ];
    let batch_result = client.upload_files_batch(batch_upload_requests).await;
    match &batch_result {
        Ok(responses) => debug!("批量上傳成功，共 {} 個文件", responses.len()),
        Err(e) => warn!("批量上傳失敗: {}", e),
    }
    assert!(batch_result.is_ok(), "批量上傳應該成功");
    if let Ok(responses) = batch_result {
        assert!(!responses.is_empty(), "應該至少上傳一個文件");
        assert!(
            !responses[0].attachment_url.is_empty(),
            "批量上傳的附件URL不應為空"
        );
    }
    // 測試帶附件的消息發送
    debug!("開始測試帶附件的消息發送");
    let file_upload_response = client
        .upload_local_file(&file_path_str, None)
        .await
        .expect("文件上傳失敗");
    let request = ChatRequest {
        version: "1.1".to_string(),
        r#type: "query".to_string(),
        query: vec![ChatMessage {
            role: "user".to_string(),
            content: "這是附加了一個文件的消息，請分析文件內容".to_string(),
            content_type: "text/markdown".to_string(),
            attachments: Some(vec![Attachment {
                url: file_upload_response.attachment_url,
                content_type: file_upload_response.mime_type,
            }]),
        }],
        temperature: None,
        user_id: String::new(),
        conversation_id: String::new(),
        message_id: String::new(),
        tools: None,
        tool_calls: None,
        tool_results: None,
        logit_bias: None,
        stop_sequences: None,
    };
    debug!("發送帶附件的消息請求");
    let result = client.stream_request(request).await;
    match &result {
        Ok(_) => debug!("帶附件的消息請求成功"),
        Err(e) => warn!("帶附件的消息請求失敗: {}", e),
    }
    assert!(result.is_ok(), "帶附件的消息請求應該成功");
    if let Ok(mut stream) = result {
        let mut received_response = false;
        debug!("開始處理帶附件的消息回應串流");
        while let Some(response) = stream.next().await {
            match response {
                Ok(event) => {
                    received_response = true;
                    debug!("收到帶附件消息的事件: {:?}", event);
                    // 檢查回應中是否提到了附件或文件
                    if let Some(ChatResponseData::Text { text }) = &event.data {
                        if text.contains("文件") || text.contains("內容") {
                            debug!("回應中提到了文件或內容，確認附件被處理");
                        }
                    }
                }
                Err(e) => {
                    warn!("帶附件消息的串流處理發生錯誤: {}", e);
                    panic!("帶附件消息的串流處理發生錯誤: {}", e);
                }
            }
        }
        assert!(received_response, "應該收到至少一個帶附件消息的回應");
    }
    // 測試無效的文件路徑
    debug!("開始測試無效的文件路徑");
    let invalid_path = "不存在的文件路徑.txt";
    let invalid_result = client.upload_local_file(invalid_path, None).await;
    match &invalid_result {
        Ok(_) => warn!("上傳不存在的文件卻成功了，這不符合預期"),
        Err(e) => debug!("如預期般，上傳不存在的文件失敗: {}", e),
    }
    assert!(invalid_result.is_err(), "上傳不存在的文件應該失敗");
    // 清理臨時文件
    debug!("測試完成，清理臨時文件");
    temp_dir.close().expect("無法清理臨時目錄");
}

#[test_log::test(tokio::test)]
async fn test_remote_file_upload() {
    setup();
    let access_key = get_access_key();
    debug!("建立 PoeClient 測試實例，用於遠程文件上傳測試");
    let client = PoeClient::new(
        "Claude-3.7-Sonnet",
        &access_key,
        "https://api.poe.com",
        "https://www.quora.com/poe_api/file_upload_3RD_PARTY_POST",
    );

    // 使用公開可訪問的測試文件URL
    let test_url = "https://www.w3.org/WAI/ER/tests/xhtml/testfiles/resources/pdf/dummy.pdf";

    debug!("開始測試遠程文件上傳，URL: {}", test_url);
    let upload_result = client.upload_remote_file(test_url).await;
    match &upload_result {
        Ok(response) => {
            debug!("遠程文件上傳成功，附件URL: {}", response.attachment_url);
            debug!("文件MIME類型: {}", response.mime_type.clone().unwrap());
            debug!("文件大小: {} 字節", response.size.unwrap());
        }
        Err(e) => warn!("遠程文件上傳失敗: {}", e),
    }

    // 注意：由於遠程服務器可能不可靠，我們不強制斷言這必須成功
    // 但如果成功，我們檢查回應格式是否正確
    if let Ok(response) = upload_result {
        assert!(!response.attachment_url.is_empty(), "附件URL不應為空");
        assert!(!response.mime_type.unwrap().is_empty(), "MIME類型不應為空");
        assert!(response.size.unwrap() > 0, "文件大小應大於0");
        debug!("遠程文件上傳測試完成");
    } else {
        debug!("遠程文件上傳失敗，這可能是網絡問題或服務限制");
    }

    // 測試無效的URL
    debug!("測試無效的URL");
    let invalid_url = "invalid-url";
    let invalid_result = client.upload_remote_file(invalid_url).await;
    match &invalid_result {
        Ok(_) => warn!("上傳無效URL卻成功了，這不符合預期"),
        Err(e) => debug!("如預期般，上傳無效URL失敗: {}", e),
    }
    assert!(invalid_result.is_err(), "上傳無效URL應該失敗");
}

#[test_log::test(tokio::test)]
async fn test_get_v1_model_list() {
    setup();
    let access_key = get_access_key();
    debug!("開始測試獲取 v1/models 模型列表");

    let client = PoeClient::new(
        "Claude-3.7-Sonnet",
        &access_key,
        "https://api.poe.com",
        "https://www.quora.com/poe_api/file_upload_3RD_PARTY_POST",
    );
    let result = client.get_v1_model_list().await;

    match &result {
        Ok(models) => debug!(
            "成功獲取 v1/models 模型列表，共 {} 個模型",
            models.data.len()
        ),
        Err(e) => warn!("獲取 v1/models 模型列表失敗: {}", e),
    }

    match result {
        Ok(models) => {
            assert!(!models.data.is_empty(), "v1/models 模型列表不應為空");
            debug!("成功獲取 {} 個 v1 模型", models.data.len());

            // 驗證第一個模型的基本資訊
            if let Some(first_model) = models.data.first() {
                assert!(!first_model.id.is_empty(), "模型 ID 不應為空");
                assert_eq!(first_model.object, "model", "模型類型應為 'model'");
                assert_eq!(first_model.owned_by, "poe", "模型擁有者應為 'poe'");

                debug!("第一個 v1 模型資訊：");
                debug!("ID: {}", first_model.id);
                debug!("類型: {}", first_model.object);
                debug!("擁有者: {}", first_model.owned_by);
                debug!("創建時間: {}", first_model.created);
            }
        }
        Err(e) => {
            warn!("獲取 v1/models 模型列表失敗: {}", e);
            panic!("獲取 v1/models 模型列表失敗: {}", e);
        }
    }

    debug!("獲取 v1/models 模型列表測試完成");
}

// XML 解析測試用例
#[cfg(feature = "xml")]
#[tokio::test]
async fn test_xml_tool_call_detection() {
    setup();
    debug!("開始測試 XML 工具調用檢測");

    let message = ChatMessage {
        role: "assistant".to_string(),
        content: "我需要查詢天氣信息。\n\n<tool_call>\n<invoke name=\"get_weather\">\n<parameter name=\"location\">台北</parameter>\n</invoke>\n</tool_call>\n\n請稍等片刻。".to_string(),
        attachments: None,
        content_type: "text/plain".to_string(),
    };

    assert!(message.contains_xml_tool_calls(), "應該檢測到 XML 工具調用");
    debug!("XML 工具調用檢測測試完成");
}

#[cfg(feature = "xml")]
#[tokio::test]
async fn test_xml_tool_call_extraction() {
    setup();
    debug!("開始測試 XML 工具調用提取");

    let message = ChatMessage {
        role: "assistant".to_string(),
        content: "我來幫您查詢天氣。\n\n<tool_call>\n<invoke name=\"get_weather\">\n<parameter name=\"location\">台北</parameter>\n<parameter name=\"unit\">celsius</parameter>\n</invoke>\n</tool_call>\n\n正在查詢中...".to_string(),
        attachments: None,
        content_type: "text/plain".to_string(),
    };

    let tool_calls = message.extract_xml_tool_calls();

    assert_eq!(tool_calls.len(), 1, "應該提取到一個工具調用");
    assert_eq!(
        tool_calls[0].function.name, "get_weather",
        "工具名稱應該是 get_weather"
    );

    // 解析參數
    let args: serde_json::Value =
        serde_json::from_str(&tool_calls[0].function.arguments).expect("參數應該是有效的 JSON");
    assert_eq!(args["location"], "台北", "location 參數應該是台北");
    assert_eq!(args["unit"], "celsius", "unit 參數應該是 celsius");

    debug!("XML 工具調用提取測試完成");
}

#[cfg(feature = "xml")]
#[tokio::test]
async fn test_multiple_xml_tool_calls() {
    setup();
    debug!("開始測試多個 XML 工具調用");

    let message = ChatMessage {
        role: "assistant".to_string(),
        content: "我需要執行兩個操作：\n\n<tool_call>\n<invoke name=\"get_weather\">\n<parameter name=\"location\">台北</parameter>\n</invoke>\n</tool_call>\n\n<tool_call>\n<invoke name=\"calculate\">\n<parameter name=\"expression\">2+2</parameter>\n</invoke>\n</tool_call>\n\n請稍等。".to_string(),
        attachments: None,
        content_type: "text/plain".to_string(),
    };

    let tool_calls = message.extract_xml_tool_calls();

    assert_eq!(tool_calls.len(), 2, "應該提取到兩個工具調用");
    assert_eq!(
        tool_calls[0].function.name, "get_weather",
        "第一個工具應該是 get_weather"
    );
    assert_eq!(
        tool_calls[1].function.name, "calculate",
        "第二個工具應該是 calculate"
    );

    // 檢查第一個工具調用的參數
    let args1: serde_json::Value = serde_json::from_str(&tool_calls[0].function.arguments)
        .expect("第一個工具的參數應該是有效的 JSON");
    assert_eq!(
        args1["location"], "台北",
        "第一個工具的 location 參數應該是台北"
    );

    // 檢查第二個工具調用的參數
    let args2: serde_json::Value = serde_json::from_str(&tool_calls[1].function.arguments)
        .expect("第二個工具的參數應該是有效的 JSON");
    assert_eq!(
        args2["expression"], "2+2",
        "第二個工具的 expression 參數應該是 2+2"
    );

    debug!("多個 XML 工具調用測試完成");
}

#[cfg(feature = "xml")]
#[tokio::test]
async fn test_xml_tool_call_with_complex_parameters() {
    setup();
    debug!("開始測試複雜參數的 XML 工具調用");

    let message = ChatMessage {
        role: "assistant".to_string(),
        content: "<tool_call>\n<invoke name=\"send_email\">\n<parameter name=\"to\">user@example.com</parameter>\n<parameter name=\"subject\">測試郵件</parameter>\n<parameter name=\"body\">這是一封測試郵件，包含特殊字符：&lt;test&gt;</parameter>\n<parameter name=\"priority\">high</parameter>\n</invoke>\n</tool_call>".to_string(),
        attachments: None,
        content_type: "text/plain".to_string(),
    };

    let tool_calls = message.extract_xml_tool_calls();

    assert_eq!(tool_calls.len(), 1, "應該提取到一個工具調用");
    assert_eq!(
        tool_calls[0].function.name, "send_email",
        "工具名稱應該是 send_email"
    );

    let args: serde_json::Value =
        serde_json::from_str(&tool_calls[0].function.arguments).expect("參數應該是有效的 JSON");
    assert_eq!(args["to"], "user@example.com", "to 參數應該正確");
    assert_eq!(args["subject"], "測試郵件", "subject 參數應該正確");
    assert_eq!(
        args["body"], "這是一封測試郵件，包含特殊字符：<test>",
        "body 參數應該正確解碼 XML 實體"
    );
    assert_eq!(args["priority"], "high", "priority 參數應該正確");

    debug!("複雜參數的 XML 工具調用測試完成");
}

#[cfg(feature = "xml")]
#[tokio::test]
async fn test_no_xml_tool_calls() {
    setup();
    debug!("開始測試沒有 XML 工具調用的情況");

    let message = ChatMessage {
        role: "assistant".to_string(),
        content: "這是一個普通的回應，沒有工具調用。".to_string(),
        attachments: None,
        content_type: "text/plain".to_string(),
    };

    assert!(
        !message.contains_xml_tool_calls(),
        "不應該檢測到 XML 工具調用"
    );
    let tool_calls = message.extract_xml_tool_calls();
    assert!(tool_calls.is_empty(), "不應該提取到任何工具調用");

    debug!("沒有 XML 工具調用的測試完成");
}

#[cfg(feature = "xml")]
#[tokio::test]
async fn test_xml_tool_call_with_empty_parameters() {
    setup();
    debug!("開始測試沒有參數的 XML 工具調用");

    let message = ChatMessage {
        role: "assistant".to_string(),
        content:
            "執行無參數工具。\n\n<tool_call>\n<invoke name=\"get_time\">\n</invoke>\n</tool_call>"
                .to_string(),
        attachments: None,
        content_type: "text/plain".to_string(),
    };

    let tool_calls = message.extract_xml_tool_calls();

    assert_eq!(tool_calls.len(), 1, "應該提取到一個工具調用");
    assert_eq!(
        tool_calls[0].function.name, "get_time",
        "工具名稱應該是 get_time"
    );

    let args: serde_json::Value =
        serde_json::from_str(&tool_calls[0].function.arguments).expect("參數應該是有效的 JSON");
    assert!(args.is_object(), "參數應該是一個空對象");
    assert_eq!(args.as_object().unwrap().len(), 0, "參數對象應該是空的");

    debug!("沒有參數的 XML 工具調用測試完成");
}

#[cfg(feature = "xml")]
#[tokio::test]
async fn test_xml_tool_call_parsing_error_handling() {
    setup();
    debug!("開始測試 XML 工具調用解析錯誤處理");

    // 測試格式錯誤的 XML
    let message_with_invalid_xml = ChatMessage {
        role: "assistant".to_string(),
        content: "格式錯誤的 XML。\n\n<tool_call>\n<invoke name=\"get_weather\">\n<parameter name=\"location\">台北\n</invoke>\n</tool_call>".to_string(),
        attachments: None,
        content_type: "text/plain".to_string(),
    };

    // 即使 XML 格式有問題，函數也應該能夠處理而不崩潰
    let tool_calls = message_with_invalid_xml.extract_xml_tool_calls();
    // 由於 XML 格式錯誤，可能無法正確解析，但不應該崩潰
    debug!("格式錯誤的 XML 解析結果：{} 個工具調用", tool_calls.len());

    debug!("XML 工具調用解析錯誤處理測試完成");
}

#[cfg(feature = "xml")]
#[tokio::test]
async fn test_xml_entity_decoding() {
    setup();
    debug!("開始測試 XML 實體解碼");

    let message = ChatMessage {
        role: "assistant".to_string(),
        content: "<tool_call>\n<invoke name=\"test_tool\">\n<parameter name=\"text\">&lt;hello&gt; &amp; &quot;world&quot; &apos;test&apos;</parameter>\n</invoke>\n</tool_call>".to_string(),
        attachments: None,
        content_type: "text/plain".to_string(),
    };

    let tool_calls = message.extract_xml_tool_calls();

    assert_eq!(tool_calls.len(), 1, "應該提取到一個工具調用");

    let args: serde_json::Value =
        serde_json::from_str(&tool_calls[0].function.arguments).expect("參數應該是有效的 JSON");
    assert_eq!(
        args["text"], "<hello> & \"world\" 'test'",
        "XML 實體應該被正確解碼"
    );

    debug!("XML 實體解碼測試完成");
}

#[cfg(feature = "xml")]
#[tokio::test]
async fn test_dynamic_xml_tool_call_detection() {
    setup();
    debug!("開始測試動態 XML 工具調用檢測");

    // 創建自定義工具定義
    let custom_tools = vec![
        ChatTool {
            r#type: "function".to_string(),
            function: FunctionDefinition {
                name: "custom_weather_api".to_string(),
                description: Some("自定義天氣 API".to_string()),
                parameters: Some(FunctionParameters {
                    r#type: "object".to_string(),
                    properties: json!({
                        "city": {
                            "type": "string",
                            "description": "城市名稱"
                        }
                    }),
                    required: vec!["city".to_string()],
                }),
            },
        },
        ChatTool {
            r#type: "function".to_string(),
            function: FunctionDefinition {
                name: "send_notification".to_string(),
                description: Some("發送通知".to_string()),
                parameters: Some(FunctionParameters {
                    r#type: "object".to_string(),
                    properties: json!({
                        "message": {
                            "type": "string",
                            "description": "通知消息"
                        }
                    }),
                    required: vec!["message".to_string()],
                }),
            },
        },
    ];

    // 測試包含自定義工具標籤的消息
    let message_with_custom_tool = ChatMessage {
        role: "assistant".to_string(),
        content: "我需要查詢天氣。\n\n<custom_weather_api>\n<city>台北</city>\n</custom_weather_api>\n\n正在查詢...".to_string(),
        attachments: None,
        content_type: "text/plain".to_string(),
    };

    // 使用基於工具定義的檢測
    assert!(
        message_with_custom_tool.contains_xml_tool_calls_with_tools(&custom_tools),
        "應該檢測到自定義工具調用"
    );

    // 測試不包含任何工具標籤的消息
    let message_without_tools = ChatMessage {
        role: "assistant".to_string(),
        content: "這是一個普通的回應，沒有任何工具調用。".to_string(),
        attachments: None,
        content_type: "text/plain".to_string(),
    };

    assert!(
        !message_without_tools.contains_xml_tool_calls_with_tools(&custom_tools),
        "不應該檢測到工具調用"
    );

    debug!("動態 XML 工具調用檢測測試完成");
}

#[cfg(feature = "xml")]
#[tokio::test]
async fn test_dynamic_xml_tool_call_extraction() {
    setup();
    debug!("開始測試動態 XML 工具調用提取");

    // 創建自定義工具定義
    let custom_tools = vec![ChatTool {
        r#type: "function".to_string(),
        function: FunctionDefinition {
            name: "database_query".to_string(),
            description: Some("數據庫查詢".to_string()),
            parameters: Some(FunctionParameters {
                r#type: "object".to_string(),
                properties: json!({
                    "table": {
                        "type": "string",
                        "description": "表名"
                    },
                    "conditions": {
                        "type": "string",
                        "description": "查詢條件"
                    }
                }),
                required: vec!["table".to_string()],
            }),
        },
    }];

    // 測試包含自定義工具調用的消息
    let message = ChatMessage {
        role: "assistant".to_string(),
        content: "我需要查詢數據庫。\n\n<database_query>\n<table>users</table>\n<conditions>age > 18</conditions>\n</database_query>\n\n正在查詢...".to_string(),
        attachments: None,
        content_type: "text/plain".to_string(),
    };

    debug!("測試消息內容: {}", message.content);
    debug!(
        "是否包含 database_query 標籤: {}",
        message.content.contains("<database_query>")
    );

    // 先測試通用方法
    let general_tool_calls = message.extract_xml_tool_calls();
    debug!("通用方法提取到的工具調用數量: {}", general_tool_calls.len());

    // 再測試基於工具定義的方法
    let tool_calls = message.extract_xml_tool_calls_with_tools(&custom_tools);
    debug!("基於工具定義提取到的工具調用數量: {}", tool_calls.len());

    if !tool_calls.is_empty() {
        debug!("工具調用內容: {:?}", tool_calls[0]);
        debug!("參數字符串: {}", tool_calls[0].function.arguments);

        // 解析參數
        let args: serde_json::Value =
            serde_json::from_str(&tool_calls[0].function.arguments).expect("參數應該是有效的 JSON");
        debug!("解析後的參數: {:?}", args);

        assert_eq!(
            tool_calls[0].function.name, "database_query",
            "工具名稱應該是 database_query"
        );
        assert_eq!(args["table"], "users", "table 參數應該是 users");
        assert_eq!(
            args["conditions"], "age > 18",
            "conditions 參數應該是 age > 18"
        );
    } else {
        debug!("沒有提取到工具調用");
        panic!("應該提取到一個工具調用");
    }

    debug!("動態 XML 工具調用提取測試完成");
}

#[cfg(feature = "xml")]
#[tokio::test]
async fn test_potential_tool_name_detection() {
    setup();
    debug!("開始測試潛在工具名稱檢測");

    // 創建包含 fetch_data 工具的工具定義
    let tools_with_fetch_data = vec![ChatTool {
        r#type: "function".to_string(),
        function: FunctionDefinition {
            name: "fetch_data".to_string(),
            description: Some("獲取數據".to_string()),
            parameters: Some(FunctionParameters {
                r#type: "object".to_string(),
                properties: json!({
                    "url": {
                        "type": "string",
                        "description": "API URL"
                    }
                }),
                required: vec!["url".to_string()],
            }),
        },
    }];

    // 測試包含潛在工具名稱的消息
    let message_with_potential_tool = ChatMessage {
        role: "assistant".to_string(),
        content: "我需要執行操作。\n\n<fetch_data>\n<url>https://api.example.com</url>\n</fetch_data>\n\n正在處理...".to_string(),
        attachments: None,
        content_type: "text/plain".to_string(),
    };

    assert!(
        message_with_potential_tool.contains_xml_tool_calls_with_tools(&tools_with_fetch_data),
        "應該檢測到潛在的工具調用（fetch_data）"
    );

    // 測試包含 HTML 標籤的消息（不應該被檢測為工具調用）
    let message_with_html = ChatMessage {
        role: "assistant".to_string(),
        content: "這是一個包含 HTML 的回應：\n\n<div>\n<p>這是段落</p>\n</div>".to_string(),
        attachments: None,
        content_type: "text/plain".to_string(),
    };

    assert!(
        !message_with_html.contains_xml_tool_calls_with_tools(&tools_with_fetch_data),
        "不應該將 HTML 標籤檢測為工具調用"
    );

    // 創建包含 getUserData 工具的工具定義
    let tools_with_get_user_data = vec![ChatTool {
        r#type: "function".to_string(),
        function: FunctionDefinition {
            name: "getUserData".to_string(),
            description: Some("獲取用戶數據".to_string()),
            parameters: Some(FunctionParameters {
                r#type: "object".to_string(),
                properties: json!({
                    "userId": {
                        "type": "string",
                        "description": "用戶ID"
                    }
                }),
                required: vec!["userId".to_string()],
            }),
        },
    }];

    // 測試包含駝峰命名工具的消息
    let message_with_camel_case = ChatMessage {
        role: "assistant".to_string(),
        content: "執行操作。\n\n<getUserData>\n<userId>123</userId>\n</getUserData>".to_string(),
        attachments: None,
        content_type: "text/plain".to_string(),
    };

    assert!(
        message_with_camel_case.contains_xml_tool_calls_with_tools(&tools_with_get_user_data),
        "應該檢測到駝峰命名的工具調用（getUserData）"
    );

    debug!("潛在工具名稱檢測測試完成");
}

#[cfg(feature = "xml")]
#[tokio::test]
async fn test_mixed_tool_call_formats() {
    setup();
    debug!("開始測試混合工具調用格式");

    // 創建包含多種格式的工具定義
    let tools = vec![ChatTool {
        r#type: "function".to_string(),
        function: FunctionDefinition {
            name: "standard_tool".to_string(),
            description: Some("標準工具".to_string()),
            parameters: Some(FunctionParameters {
                r#type: "object".to_string(),
                properties: json!({
                    "param": {
                        "type": "string",
                        "description": "參數"
                    }
                }),
                required: vec!["param".to_string()],
            }),
        },
    }];

    // 測試包含多種格式的消息
    let message = ChatMessage {
        role: "assistant".to_string(),
        content: r#"我需要執行多個操作：

1. 標準格式：
<tool_call>
<invoke name="standard_tool">
<parameter name="param">value1</parameter>
</invoke>
</tool_call>

2. 簡化格式：
<standard_tool>
<param>value2</param>
</standard_tool>

正在處理..."#
            .to_string(),
        attachments: None,
        content_type: "text/plain".to_string(),
    };

    let tool_calls = message.extract_xml_tool_calls_with_tools(&tools);

    // 應該能夠解析兩種格式的工具調用
    assert!(tool_calls.len() >= 1, "應該至少提取到一個工具調用");

    // 檢查是否包含標準工具
    let has_standard_tool = tool_calls
        .iter()
        .any(|call| call.function.name == "standard_tool");
    assert!(has_standard_tool, "應該包含 standard_tool 調用");

    debug!("混合工具調用格式測試完成");
}

#[cfg(feature = "xml")]
#[tokio::test]
async fn test_remove_xml_tool_calls_with_tool_cells() {
    setup();
    debug!("開始測試移除包含工具調用的 XML");

    use crate::client::PoeClient;

    // 測試包含工具調用的文本
    let text_with_tools = r#"我需要查詢天氣信息。

<tool_call>
<invoke name="get_weather">
<parameter name="location">台北</parameter>
<parameter name="unit">celsius</parameter>
</invoke>
</tool_call>

請稍等片刻，我正在為您查詢台北的天氣。"#;

    let cleaned_text = PoeClient::remove_xml_tool_calls(text_with_tools);

    // 應該移除工具調用部分
    assert!(
        !cleaned_text.contains("<tool_call>"),
        "應該移除 tool_call 標籤"
    );
    assert!(!cleaned_text.contains("<invoke"), "應該移除 invoke 標籤");
    assert!(
        !cleaned_text.contains("<parameter"),
        "應該移除 parameter 標籤"
    );
    assert!(
        cleaned_text.contains("我需要查詢天氣信息。"),
        "應該保留普通文本"
    );
    assert!(
        cleaned_text.contains("請稍等片刻，我正在為您查詢台北的天氣。"),
        "應該保留普通文本"
    );

    debug!("移除包含工具調用的 XML 測試完成");
}

#[cfg(feature = "xml")]
#[tokio::test]
async fn test_remove_xml_tool_calls_without_tool_cells() {
    setup();
    debug!("開始測試移除不包含工具調用的文本");

    use crate::client::PoeClient;

    // 測試不包含工具調用的文本
    let text_without_tools = r#"這是一個普通的回應，沒有任何工具調用。
我可以為您提供一般性的幫助和信息。"#;

    let cleaned_text = PoeClient::remove_xml_tool_calls(text_without_tools);

    // 應該保持原文本不變
    assert_eq!(
        cleaned_text, text_without_tools,
        "不包含工具調用的文本應該保持不變"
    );

    debug!("移除不包含工具調用的文本測試完成");
}
