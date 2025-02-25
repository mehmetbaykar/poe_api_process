# Poe Bot Process

這是一個用 Rust 實現的 Poe API 客戶端庫。它允許您與 Poe API 平台進行交互，發送查詢請求並接收回應。

## 功能

- 流式接收 bot 回應
- 獲取可用模型列表

## 安裝

在您的 `Cargo.toml` 文件中添加以下依賴：
```toml
[dependencies]
poe_api_process = "0.1.5"
```

Or
```bash
cargo add poe_api_process
```

## 使用方法

### 創建客戶端並發送請求

```rust
use poe_api_process::{PoeClient, QueryRequest, ProtocolMessage, EventType};
use futures_util::StreamExt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = PoeClient::new("your_bot_name", "your_access_key");
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

    let mut stream = client.stream_request(request).await?;

    while let Some(response) = stream.next().await {
        match response {
            Ok(event) => match event.event {
                EventType::Text => {
                    if let Some(partial) = event.data {
                        println!("收到文字: {}", partial.text);
                    }
                },
                EventType::ReplaceResponse => {
                    if let Some(partial) = event.data {
                        println!("替換回應: {}", partial.text);
                    }
                },
                EventType::Error => {
                    if let Some(error) = event.error {
                        eprintln!("伺服器錯誤: {}", error.text);
                        if error.allow_retry {
                            println!("可以重試請求");
                        }
                    }
                },
                EventType::Done => {
                    println!("對話完成");
                    break;
                },
            },
            Err(e) => eprintln!("錯誤: {}", e),
        }
    }

    Ok(())
}
```

### 獲取可用模型列表

```rust
use poe_api_process::get_model_list;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let models = get_model_list(Some("zh-Hant")).await?;
    for (index, model) in models.data.iter().enumerate() {
        println!("Model ID {} - {}", index + 1, model.id);
    }

    Ok(())
}
```

## 注意事項

請確保您擁有可使用的[Poe API 訪問密鑰](https://poe.com/api_key)。

使用 stream_request 時，請提供有效的 bot 名稱和訪問密鑰。

get_model_list 不需要訪問密鑰，可以直接使用。