# Poe API Process

[[English](https://github.com/jeromeleong/poe_api_process/blob/master/README_EN.md)|[繁體中文](https://github.com/jeromeleong/poe_api_process/blob/master/README.md)|[简体中文](https://github.com/jeromeleong/poe_api_process/blob/master/README_CN.md)]

這是一個用 Rust 實現的 Poe API 客戶端庫。它允許您與 Poe API 平台進行互動，發送查詢請求並接收回應。

## 功能

- 流式接收 bot 回應
- 獲取可用模型列表
- 支援工具調用 (Tool Calls)
- 支援檔案上傳與附件傳送

## 安裝

在您的 `Cargo.toml` 文件中添加以下依賴：

```toml
[dependencies]
poe_api_process = "0.2.0"
```

或使用 cargo 指令添加：

```bash
cargo add poe_api_process
```

## 使用方法

### 創建客戶端並發送請求

```rust
use poe_api_process::{PoeClient, ChatRequest, ChatMessage, ChatEventType};
use futures_util::StreamExt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = PoeClient::new("Claude-3.7-Sonnet", "your_access_key");
    
    let request = ChatRequest {
        version: "1.1".to_string(),
        r#type: "query".to_string(),
        query: vec![ChatMessage {
            role: "user".to_string(),
            content: "你好".to_string(),
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
    
    let mut stream = client.stream_request(request).await?;
    
    while let Some(response) = stream.next().await {
        match response {
            Ok(event) => match event.event {
                ChatEventType::Text => {
                    if let Some(data) = event.data {
                        if let crate::types::ChatResponseData::Text { text } = data {
                            println!("收到文字: {}", text);
                        }
                    }
                },
                ChatEventType::ReplaceResponse => {
                    if let Some(data) = event.data {
                        if let crate::types::ChatResponseData::Text { text } = data {
                            println!("替換回應: {}", text);
                        }
                    }
                },
                ChatEventType::Error => {
                    if let Some(data) = event.data {
                        if let crate::types::ChatResponseData::Error { text, allow_retry } = data {
                            eprintln!("伺服器錯誤: {}", text);
                            if allow_retry {
                                println!("可以重試請求");
                            }
                        }
                    }
                },
                ChatEventType::Done => {
                    println!("對話完成");
                    break;
                },
                ChatEventType::Json => {
                    println!("收到 JSON 事件");
                },
            },
            Err(e) => eprintln!("錯誤: {}", e),
        }
    }
    
    Ok(())
}
```

### 工具調用 (Tool Call)

- **工具調用 (Tool Call)**: 允許 AI 模型請求執行特定的工具或函數。例如，AI 可能需要查詢天氣、搜索網頁或執行計算等操作。
- **工具結果 (Tool Result)**: 工具執行後返回的結果，將被發送回 AI 模型以繼續對話。

在建立請求時，可以指定可用的工具：

```rust
use serde_json::json;
use poe_api_process::{ChatTool, FunctionDefinition, FunctionParameters};

let request = ChatRequest {
    // 其他欄位...
    tools: Some(vec![ChatTool {
        r#type: "function".to_string(),
        function: FunctionDefinition {
            name: "get_weather".to_string(),
            description: "獲取指定城市的天氣資訊".to_string(),
            parameters: FunctionParameters {
                r#type: "object".to_string(),
                properties: json!({
                    "city": {
                        "type": "string",
                        "description": "城市名稱"
                    }
                }),
                required: vec!["city".to_string()],
            },
        },
    }]),
    // 其他欄位...
};
```

當 AI 模型返回工具調用時，您可以處理並提供結果：

```rust
use poe_api_process::{ChatToolResult, ChatResponseData};

while let Some(response) = stream.next().await {
    match response {
        Ok(event) => match event.event {
            ChatEventType::Json => {
                if let Some(ChatResponseData::ToolCalls(tool_calls)) = event.data {
                    println!("收到工具調用請求: {:?}", tool_calls);
                    
                    // 處理工具調用
                    let tool_results = vec![ChatToolResult {
                        role: "tool".to_string(),
                        tool_call_id: tool_calls[0].id.clone(),
                        name: tool_calls[0].function.name.clone(),
                        content: r#"{"temperature": 25, "condition": "晴天"}"#.to_string(),
                    }];
                    
                    // 發送工具結果回 AI
                    let mut result_stream = client.send_tool_results(
                        request.clone(),
                        tool_calls,
                        tool_results
                    ).await?;
                    
                    // 處理後續回應...
                    while let Some(result_response) = result_stream.next().await {
                        // 處理回應...
                    }
                }
            },
            // 其他事件處理...
        },
        Err(e) => eprintln!("錯誤: {}", e),
    }
}
```

### 檔案上傳與使用附件

本庫支援上傳本地或遠端檔案，並在請求中附加這些檔案：

```rust
use poe_api_process::{Attachment, FileUploadRequest};

// 上傳單個本地檔案
let upload_result = client.upload_local_file("path/to/document.pdf", mime_type: None).await?;
println!("檔案已上傳，URL: {}", upload_result.attachment_url);

// 上傳遠端檔案 (通過 URL)
let remote_upload = client.upload_remote_file("https://example.com/document.pdf").await?;

// 批次上傳多個檔案
let batch_results = client.upload_files_batch(vec![
    FileUploadRequest::LocalFile { file: "path/to/first.pdf".to_string() , mime_type: None},
    FileUploadRequest::RemoteFile { download_url: "https://example.com/second.pdf".to_string() },
]).await?;

// 在請求中附加檔案
let request = ChatRequest {
    // 其他欄位...
    query: vec![ChatMessage {
        role: "user".to_string(),
        content: "請分析這份文件".to_string(),
        content_type: "text/markdown".to_string(),
        attachments: Some(vec![Attachment {
            url: upload_result.attachment_url,
            content_type: upload_result.mime_type,
        }]),
    }],
    // 其他欄位...
};
```

### 獲取可用模型列表

```rust
use poe_api_process::get_model_list;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 獲取繁體中文版的模型列表
    let models = get_model_list(Some("zh-Hant")).await?;
    
    println!("可用模型列表:");
    for (index, model) in models.data.iter().enumerate() {
        println!("{}. {}", index + 1, model.id);
    }
    
    Ok(())
}
```

## 除錯功能

啟用 trace 功能可以獲得詳細的日誌輸出：

```toml
[dependencies]
poe_api_process = { version = "0.2.0", features = ["trace"] }
```

## 注意事項

- 請確保您擁有可使用的 [Poe API 訪問密鑰](https://poe.com/api_key)。
- 使用 `stream_request` 時，請提供有效的 bot 名稱和訪問密鑰。
- `get_model_list` 不需要訪問密鑰，可以直接使用。
- 檔案上傳功能受到 Poe 平台的檔案大小和類型限制。