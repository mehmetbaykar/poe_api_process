use crate::{get_model_list, PoeClient, ProtocolMessage, QueryRequest};
use dotenv::dotenv;
use futures_util::StreamExt;
use std::env;
use tracing::{debug, warn};

fn setup() {
    // 載入環境變數
    dotenv().ok();
    debug!("測試環境設定完成");
}

#[test_log::test(tokio::test)]
async fn test_stream_request() {
    setup();

    let access_key = match env::var("POE_ACCESS_KEY") {
        Ok(key) => {
            debug!("成功讀取 POE_ACCESS_KEY 環境變數");
            key
        }
        Err(_) => {
            warn!("無法讀取 POE_ACCESS_KEY 環境變數");
            panic!("需要在 .env 檔案中設置 POE_ACCESS_KEY");
        }
    };

    debug!("建立 PoeClient 測試實例");
    let client = PoeClient::new("Claude-3.7-Sonnet", &access_key);

    let request = QueryRequest {
        version: "1".to_string(),
        r#type: "query".to_string(),
        query: vec![ProtocolMessage {
            role: "user".to_string(),
            content: "你好".to_string(),
            content_type: "text/markdown".to_string(),
        }],
        temperature: None,
        user_id: String::new(),
        conversation_id: String::new(),
        message_id: String::new(),
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
