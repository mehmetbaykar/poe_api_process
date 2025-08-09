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
    let client = PoeClient::new("Claude-3.7-Sonnet", &access_key, "https://api.poe.com", "https://www.quora.com/poe_api/file_upload_3RD_PARTY_POST");

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
    let client = PoeClient::new("Claude-3.7-Sonnet", &access_key, "https://api.poe.com", "https://www.quora.com/poe_api/file_upload_3RD_PARTY_POST");

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
    let client = PoeClient::new("GPT-4o-Mini", &access_key, "https://api.poe.com", "https://www.quora.com/poe_api/file_upload_3RD_PARTY_POST");

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
    let client = PoeClient::new("Claude-3.7-Sonnet", &access_key, "https://api.poe.com", "https://www.quora.com/poe_api/file_upload_3RD_PARTY_POST");
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
    let client = PoeClient::new("Claude-3.7-Sonnet", &access_key, "https://api.poe.com", "https://www.quora.com/poe_api/file_upload_3RD_PARTY_POST");

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
    
    let client = PoeClient::new("Claude-3.7-Sonnet", &access_key, "https://api.poe.com", "https://www.quora.com/poe_api/file_upload_3RD_PARTY_POST");
    let result = client.get_v1_model_list().await;

    match &result {
        Ok(models) => debug!("成功獲取 v1/models 模型列表，共 {} 個模型", models.data.len()),
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
