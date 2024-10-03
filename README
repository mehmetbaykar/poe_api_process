# Poe Bot Process

這是一個用 Rust 實現的 Poe API 客戶端庫。它允許您與 Poe API 平台進行交互,發送查詢請求並接收回應。

## 功能

- 流式接收 bot 回應
- 獲取 bot 設置
- 獲取可用模型列表

## 使用方法

```rust
use poe_bot_process::{PoeClient, PoeBot, QueryRequest};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = PoeClient::new("your_bot_name", "your_access_key");

    // 獲取模型列表
    let models = client.get_model_list().await?;
    println!("可用的模型: {:?}", models);

}
```

注意事項
請確保您有有效的 [Poe API](https://poe.com/api_key) 訪問密鑰。
