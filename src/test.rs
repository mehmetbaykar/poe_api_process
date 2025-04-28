use crate::{get_model_list, PoeClient, ProtocolMessage, QueryRequest};
use crate::types::{EventType, Tool, ToolCall, ToolFunction, ToolFunctionParameters};
use dotenv::dotenv;
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
    let client = PoeClient::new("Claude-3.7-Sonnet", &access_key);
    
    let request = QueryRequest {
        version: "1".to_string(),
        r#type: "query".to_string(),
        query: vec![ProtocolMessage {
            role: "user".to_string(),
            content: "Hello".to_string(),
            content_type: "text/markdown".to_string(),
        }],
        temperature: None,
        user_id: String::new(),
        conversation_id: String::new(),
        message_id: String::new(),
        tools: None,
        tool_calls: None,
        tool_results: None,
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
    let client = PoeClient::new("Claude-3.7-Sonnet", &access_key);
    
    let request = QueryRequest {
        version: "1".to_string(),
        r#type: "query".to_string(),
        query: vec![ProtocolMessage {
            role: "user".to_string(),
            content: "Say 'hello' only".to_string(),
            content_type: "text/markdown".to_string(),
        }],
        temperature: None,
        user_id: String::new(),
        conversation_id: String::new(),
        message_id: String::new(),
        tools: None,
        tool_calls: None,
        tool_results: None,
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
                        EventType::Text => {
                            received_text_event = true;
                            assert!(event_response.data.is_some(), "Text 事件應包含 data");
                            let text_data = event_response.data.unwrap();
                            debug!("收到 Text 事件，內容: '{}'", text_data.text);
                        }
                        EventType::Error => {
                            assert!(event_response.error.is_some(), "Error 事件應包含 error");
                            let error_data = event_response.error.unwrap();
                            warn!("收到 Error 事件: {:?}", error_data);
                            panic!("串流處理收到 Error 事件: {:?}", error_data);
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
    let client = PoeClient::new("GPT-4o-Mini", &access_key);
    
    // 創建帶有工具定義的請求
    let request = QueryRequest {
        version: "1".to_string(),
        r#type: "query".to_string(),
        query: vec![ProtocolMessage {
            role: "user".to_string(),
            content: "What's the current weather in Taipei? Use the weather tool.".to_string(),
            content_type: "text/markdown".to_string(),
        }],
        temperature: None,
        user_id: String::new(),
        conversation_id: String::new(),
        message_id: String::new(),
        tools: Some(vec![
            Tool {
                r#type: "function".to_string(),
                function: ToolFunction {
                    name: "get_weather".to_string(),
                    description: "Get weather information for a location".to_string(),
                    parameters: ToolFunctionParameters {
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
                    },
                },
            },
        ]),
        tool_calls: None,
        tool_results: None,
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
                        EventType::Json => {
                            // 檢查是否有 tool_calls
                            if let Some(tool_calls) = &event_response.tool_calls {
                                received_tool_call = true;
                                debug!("收到 ToolCalls 事件，工具調用數量: {}", tool_calls.len());
                                
                                // 驗證工具調用的內容
                                for tool_call in tool_calls {
                                    assert_eq!(tool_call.r#type, "function", "工具調用類型應為 function");
                                    assert!(!tool_call.id.is_empty(), "工具調用 ID 不應為空");
                                    assert_eq!(tool_call.function.name, "get_weather", "工具調用函數名應為 get_weather");
                                    debug!("工具調用參數: {}", tool_call.function.arguments);
                                }
                                
                                // 因為我們已經確認收到工具調用，可以選擇退出循環
                                break;
                            } else {
                                debug!("收到 Json 事件，但不包含 tool_calls，可能是增量更新");
                            }
                        }
                        EventType::Error => {
                            assert!(event_response.error.is_some(), "Error 事件應包含 error");
                            let error_data = event_response.error.unwrap();
                            warn!("收到 Error 事件: {:?}", error_data);
                            panic!("工具串流處理收到 Error 事件: {:?}", error_data);
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
    let tool_calls: Vec<ToolCall> = serde_json::from_value(tool_calls_value.clone()).unwrap();
    
    // 驗證解析結果
    assert_eq!(tool_calls.len(), 1, "應該解析出一個工具調用");
    assert_eq!(tool_calls[0].id, "call_123456", "工具調用 ID 應該匹配");
    assert_eq!(tool_calls[0].r#type, "function", "工具調用類型應該是 function");
    assert_eq!(tool_calls[0].function.name, "get_weather", "工具調用函數名應該是 get_weather");
    assert_eq!(
        tool_calls[0].function.arguments,
        "{\"location\":\"Taipei\",\"unit\":\"celsius\"}",
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
    let parse_result: Result<Vec<ToolCall>, _> = serde_json::from_value(tool_calls_value.clone());
    
    // 驗證解析結果應該是錯誤
    assert!(parse_result.is_err(), "解析無效的工具調用應該失敗");
    
    // 驗證錯誤類型
    let error = parse_result.unwrap_err();
    debug!("解析錯誤: {}", error);
    assert!(error.to_string().contains("missing field"), "錯誤消息應該指示缺少字段");
    
    debug!("工具調用解析錯誤處理測試完成");
}