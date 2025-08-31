# Poe API Process

[[English](https://github.com/jeromeleong/poe_api_process/blob/master/README_EN.md)|[繁體中文](https://github.com/jeromeleong/poe_api_process/blob/master/README.md)|[简体中文](https://github.com/jeromeleong/poe_api_process/blob/master/README_CN.md)]

This is a Rust implementation of a Poe API client library. It allows you to interact with the Poe API platform, send query requests, and receive responses.

## Features
- Stream bot responses
- Get list of available models (supports traditional API and v1/models API)
- Support for Tool Calls
- Support for file uploads and attachments
- Support for XML format tool calls (optional feature)
- Flexible URL configuration

## Installation

Add this dependency to your `Cargo.toml` file:

```toml
[dependencies]
poe_api_process = "0.4.4"
```

Or use the cargo command:

```bash
cargo add poe_api_process
```

## Usage

### Create a client and send requests

```rust
use poe_api_process::{PoeClient, ChatRequest, ChatMessage, ChatEventType};
use futures_util::StreamExt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
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
    
    let mut stream = client.stream_request(request).await?;
    
    while let Some(response) = stream.next().await {
        match response {
            Ok(event) => match event.event {
                ChatEventType::Text => {
                    if let Some(data) = event.data {
                        if let crate::types::ChatResponseData::Text { text } = data {
                            println!("Received text: {}", text);
                        }
                    }
                },
                ChatEventType::ReplaceResponse => {
                    if let Some(data) = event.data {
                        if let crate::types::ChatResponseData::Text { text } = data {
                            println!("Replace response: {}", text);
                        }
                    }
                },
                ChatEventType::Error => {
                    if let Some(data) = event.data {
                        if let crate::types::ChatResponseData::Error { text, allow_retry } = data {
                            eprintln!("Server error: {}", text);
                            if allow_retry {
                                println!("Request can be retried");
                            }
                        }
                    }
                },
                ChatEventType::Done => {
                    println!("Conversation complete");
                    break;
                ChatEventType::Json => {
                    println!("Received JSON event");
                },
                ChatEventType::File => {
                    if let Some(data) = event.data {
                        if let crate::types::ChatResponseData::File(file_data) = data {
                            println!("Received file: {} ({})", file_data.name, file_data.url);
                        }
                    }
                },
                },
            },
            Err(e) => eprintln!("Error: {}", e),
        }
    }
    
    Ok(())
}
```

### Tool Calls

PS: Native BOT interface tool calls only support a limited number of models and have strict formatting requirements. It is recommended to use the XML Feature.

- **Tool Call**: Allows AI models to request execution of specific tools or functions. For example, AI might need to query weather, search the web, or perform calculations.
- **Tool Result**: The result returned after tool execution, which will be sent back to the AI model to continue the conversation.

When creating a request, you can specify available tools:

```rust
use serde_json::json;
use poe_api_process::{ChatTool, FunctionDefinition, FunctionParameters};

let request = ChatRequest {
    // Other fields...
    tools: Some(vec![ChatTool {
        r#type: "function".to_string(),
        function: FunctionDefinition {
            name: "get_weather".to_string(),
            description: "Get weather information for a specified city".to_string(),
            parameters: FunctionParameters {
                r#type: "object".to_string(),
                properties: json!({
                    "city": {
                        "type": "string",
                        "description": "City name"
                    }
                }),
                required: vec!["city".to_string()],
            },
        },
    }]),
    // Other fields...
};
```

When the AI model returns a tool call, you can process it and provide results:

```rust
use poe_api_process::{ChatToolResult, ChatResponseData};

while let Some(response) = stream.next().await {
    match response {
        Ok(event) => match event.event {
            ChatEventType::Json => {
                if let Some(ChatResponseData::ToolCalls(tool_calls)) = event.data {
                    println!("Received tool call request: {:?}", tool_calls);
                    
                    // Process tool call
                    let tool_results = vec![ChatToolResult {
                        role: "tool".to_string(),
                        tool_call_id: tool_calls[0].id.clone(),
                        name: tool_calls[0].function.name.clone(),
                        content: r#"{"temperature": 25, "condition": "sunny"}"#.to_string(),
                    }];
                    
                    // Send tool results back to AI
                    let mut result_stream = client.send_tool_results(
                        request.clone(),
                        tool_calls,
                        tool_results
                    ).await?;
                    
                    // Process subsequent responses...
                    while let Some(result_response) = result_stream.next().await {
                        // Handle responses...
                    }
                }
            },
            // Handle other events...
        },
        Err(e) => eprintln!("Error: {}", e),
    }
}
```

#### XML Tool Calls

Enable the xml feature to use tool calls in XML format, automatically handling XML content without changing existing code:

```toml
[dependencies]
poe_api_process = { version = "0.4.4", features = ["xml"] }
```

### File Upload and Attachments

This library supports uploading local or remote files and attaching them to requests:

```rust
use poe_api_process::{Attachment, FileUploadRequest};

// Upload a single local file
let upload_result = client.upload_local_file("path/to/document.pdf", mime_type: None).await?;
println!("File uploaded, URL: {}", upload_result.attachment_url);

// Upload a remote file (via URL)
let remote_upload = client.upload_remote_file("https://example.com/document.pdf").await?;

// Batch upload multiple files
let batch_results = client.upload_files_batch(vec![
    FileUploadRequest::LocalFile { file: "path/to/first.pdf".to_string() , mime_type: None},
    FileUploadRequest::RemoteFile { download_url: "https://example.com/second.pdf".to_string() },
]).await?;

// Attach files to a request
let request = ChatRequest {
    // Other fields...
    query: vec![ChatMessage {
        role: "user".to_string(),
        content: "Please analyze this document".to_string(),
        content_type: "text/markdown".to_string(),
        attachments: Some(vec![Attachment {
            url: upload_result.attachment_url,
            content_type: upload_result.mime_type,
        }]),
    }],
    // Other fields...
};
```

### Get Available Model List

#### Traditional Model List API (no token required)

```rust
use poe_api_process::get_model_list;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Get the model list in English
    let models = get_model_list(Some("en")).await?;
    
    println!("Available models:");
    for (index, model) in models.data.iter().enumerate() {
        println!("{}. {}", index + 1, model.id);
    }
    
    Ok(())
}
```

#### v1/models API (token required)

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
    
    // Get v1/models API model list
    let v1_models = client.get_v1_model_list().await?;
    
    println!("v1 API model list:");
    for (index, model) in v1_models.data.iter().enumerate() {
        println!("{}. {} (created: {})", index + 1, model.id, model.created);
    }
    
    Ok(())
}
```

## Debug Features

Enable trace functionality for detailed log output:

```toml
[dependencies]
poe_api_process = { version = "0.4.4", features = ["trace"] }
```

## v0.3.0 Version Changes

### Breaking Changes
- **PoeClient::new()** now requires four parameters: `bot_name`, `access_key`, `poe_base_url`, `poe_file_upload_url`
- Added **get_v1_model_list()** method as a PoeClient instance method
- Automatic URL trailing slash normalization

### Migration Guide
```rust
// v0.2.x version
let client = PoeClient::new("Claude-3.7-Sonnet", "your_access_key");

// v0.3.0+ version
let client = PoeClient::new(
    "Claude-3.7-Sonnet",
    "your_access_key",
    "https://api.poe.com",
    "https://www.quora.com/poe_api/file_upload_3RD_PARTY_POST"
);
```

## Notes

- Ensure you have a usable [Poe API access key](https://poe.com/api_key).
- When using `stream_request`, provide a valid bot name and access key.
- `get_model_list` doesn't require an access key and can be used directly.
- `get_v1_model_list` requires an access key and is called as a PoeClient method.
- File upload functionality is subject to Poe platform's file size and type limitations.