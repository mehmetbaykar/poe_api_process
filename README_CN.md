# Poe API Process
[[English](https://github.com/jeromeleong/poe_api_process/blob/master/README_EN.md)|[繁體中文](https://github.com/jeromeleong/poe_api_process/blob/master/README.md)|[简体中文]]

这是一个用 Rust 实现的 Poe API 客户端库。它允许您与 Poe API 平台进行交互，发送查询请求并接收响应。

## 功能
- 流式接收 bot 响应
- 获取可用模型列表（支持传统 API 和 v1/models API）
- 支持工具调用 (Tool Calls)
- 支持文件上传与附件传送
- 支持 XML 格式工具调用（可选功能）
- 灵活的 URL 配置

## 安装
在您的 `Cargo.toml` 文件中添加以下依赖：
```toml
[dependencies]
poe_api_process = "0.4.4"
```
或使用 cargo 命令添加：
```bash
cargo add poe_api_process
```

## 使用方法
### 创建客户端并发送请求
```rust
use poe_api_process::{PoeClient, ChatRequest, ChatMessage, ChatEventType};
use futures_util::StreamExt;
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // v0.3.0 新语法：需要提供 URL 参数
    let client = PoeClient::new(
        "Claude-3.7-Sonnet",
        "your_access_key",
        "https://api.poe.com",
        "https://www.quora.com/poe_api/file_upload_3RD_PARTY_POST"
    );
    
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
                            println!("替换响应: {}", text);
                        }
                    }
                },
                ChatEventType::Error => {
                    if let Some(data) = event.data {
                        if let crate::types::ChatResponseData::Error { text, allow_retry } = data {
                            eprintln!("服务器错误: {}", text);
                            if allow_retry {
                                println!("可以重试请求");
                            }
                        }
                    }
                },
                ChatEventType::Done => {
                    println!("对话完成");
                    break;
                },
                ChatEventType::Json => {
                    println!("收到 JSON 事件");
                },
                ChatEventType::File => {
                    if let Some(data) = event.data {
                        if let crate::types::ChatResponseData::File(file_data) = data {
                            println!("收到文件: {} ({})", file_data.name, file_data.url);
                        }
                    }
                },
            },
            Err(e) => eprintln!("错误: {}", e),
        }
    }
    
    Ok(())
}
```

### 工具调用 (Tool Call)

PS: 原生BOT接口的工具调用只支持少量模型，并且使用格式严格，建议使用 XML Feature。

- **工具调用 (Tool Call)**: 允许 AI 模型请求执行特定的工具或函数。例如，AI 可能需要查询天气、搜索网页或执行计算等操作。
- **工具结果 (Tool Result)**: 工具执行后返回的结果，将被发送回 AI 模型以继续对话。

在建立请求时，可以指定可用的工具：
```rust
use serde_json::json;
use poe_api_process::{ChatTool, FunctionDefinition, FunctionParameters};
let request = ChatRequest {
    // 其他字段...
    tools: Some(vec![ChatTool {
        r#type: "function".to_string(),
        function: FunctionDefinition {
            name: "get_weather".to_string(),
            description: "获取指定城市的天气信息".to_string(),
            parameters: FunctionParameters {
                r#type: "object".to_string(),
                properties: json!({
                    "city": {
                        "type": "string",
                        "description": "城市名称"
                    }
                }),
                required: vec!["city".to_string()],
            },
        },
    }]),
    // 其他字段...
};
```

当 AI 模型返回工具调用时，您可以处理并提供结果：
```rust
use poe_api_process::{ChatToolResult, ChatResponseData};
while let Some(response) = stream.next().await {
    match response {
        Ok(event) => match event.event {
            ChatEventType::Json => {
                if let Some(ChatResponseData::ToolCalls(tool_calls)) = event.data {
                    println!("收到工具调用请求: {:?}", tool_calls);
                    
                    // 处理工具调用
                    let tool_results = vec![ChatToolResult {
                        role: "tool".to_string(),
                        tool_call_id: tool_calls[0].id.clone(),
                        name: tool_calls[0].function.name.clone(),
                        content: r#"{"temperature": 25, "condition": "晴天"}"#.to_string(),
                    }];
                    
                    // 发送工具结果回 AI
                    let mut result_stream = client.send_tool_results(
                        request.clone(),
                        tool_calls,
                        tool_results
                    ).await?;
                    
                    // 处理后续响应...
                    while let Some(result_response) = result_stream.next().await {
                        // 处理响应...
                    }
                }
            },
            // 其他事件处理...
        },
        Err(e) => eprintln!("错误: {}", e),
    }
}
```

#### XML 工具调用

启用 xml 功能可以将工具调用改为 XML 的方式使用，自动化处理XML内容，不需要改动原有代码：

```toml
[dependencies]
poe_api_process = { version = "0.4.4", features = ["xml"] }
```

### 文件上传与使用附件
本库支持上传本地或远程文件，并在请求中附加这些文件：
```rust
use poe_api_process::{Attachment, FileUploadRequest};
// 上传单个本地文件
let upload_result = client.upload_local_file("path/to/document.pdf", mime_type: None).await?;
println!("文件已上传，URL: {}", upload_result.attachment_url);
// 上传远程文件 (通过 URL)
let remote_upload = client.upload_remote_file("https://example.com/document.pdf").await?;
// 批量上传多个文件
let batch_results = client.upload_files_batch(vec![
    FileUploadRequest::LocalFile { file: "path/to/first.pdf".to_string() , mime_type: None},
    FileUploadRequest::RemoteFile { download_url: "https://example.com/second.pdf".to_string() },
]).await?;
// 在请求中附加文件
let request = ChatRequest {
    // 其他字段...
    query: vec![ChatMessage {
        role: "user".to_string(),
        content: "请分析这份文档".to_string(),
        content_type: "text/markdown".to_string(),
        attachments: Some(vec![Attachment {
            url: upload_result.attachment_url,
            content_type: upload_result.mime_type,
        }]),
    }],
    // 其他字段...
};
```

### 获取可用模型列表

#### 使用传统 API
```rust
use poe_api_process::get_model_list;
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 获取简体中文版的模型列表
    let models = get_model_list(Some("zh-Hans")).await?;
    
    println!("可用模型列表:");
    for (index, model) in models.data.iter().enumerate() {
        println!("{}. {}", index + 1, model.id);
    }
    
    Ok(())
}
```

#### 使用 v1/models API
```rust
use poe_api_process::PoeClient;
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = PoeClient::new(
        "Claude-3.7-Sonnet",
        "your_access_key",
        "https://api.poe.com",
        "https://www.quora.com/poe_api/file_upload_3RD_PARTY_POST"
    );
    
    // 获取 v1/models API 的模型列表
    let models = client.get_v1_model_list().await?;
    
    println!("v1 API 可用模型列表:");
    for (index, model) in models.data.iter().enumerate() {
        println!("{}. {} (创建时间: {})", index + 1, model.id, model.created);
    }
    
    Ok(())
}
```

## v0.3.0 版本变更

### 重大变更
- **PoeClient::new()** 现在需要四个参数：`bot_name`、`access_key`、`poe_base_url`、`poe_file_upload_url`
- 新增 **get_v1_model_list()** 方法作为 PoeClient 的实例方法
- 自动处理 URL 末尾斜线规范化

### 迁移指南
```rust
// v0.2.x 版本
let client = PoeClient::new("Claude-3.7-Sonnet", "your_access_key");

// v0.3.0+ 版本
let client = PoeClient::new(
    "Claude-3.7-Sonnet",
    "your_access_key",
    "https://api.poe.com",
    "https://www.quora.com/poe_api/file_upload_3RD_PARTY_POST"
);
```

## 调试功能
启用 trace 功能可以获得详细的日志输出：
```toml
[dependencies]
poe_api_process = { version = "0.4.4", features = ["trace"] }
```

## 注意事项
- 请确保您拥有可使用的 [Poe API 访问密钥](https://poe.com/api_key)。
- 使用 `stream_request` 时，请提供有效的 bot 名称和访问密钥。
- `get_model_list` 不需要访问密钥，可以直接使用。
- `get_v1_model_list` 需要通过 PoeClient 实例调用，需要访问密钥。
- 文件上传功能受到 Poe 平台的文件大小和类型限制。
- URL 参数会自动处理末尾的斜线，确保格式正确。