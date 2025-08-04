# poe_api_process

[![Crates.io](https://img.shields.io/crates/v/poe_api_process.svg)](https://crates.io/crates/poe_api_process)
[![Documentation](https://docs.rs/poe_api_process/badge.svg)](https://docs.rs/poe_api_process)
[![License](https://img.shields.io/crates/l/poe_api_process.svg)](LICENSE)

A Rust client library for interacting with the Poe.com API, providing streaming responses, tool calls support, and file upload capabilities.

## Features

- 🚀 **Streaming Responses** - Real-time streaming of bot responses using Server-Sent Events (SSE)
- 🛠️ **Tool Calls** - Full support for function calling with streaming tool call accumulation
- 📁 **File Uploads** - Upload local files, remote files via URL, or batch upload multiple files
- 🤖 **Multiple Bot Support** - Works with any bot available on Poe.com
- 🔍 **Model Discovery** - Query available models/bots dynamically
- 🦀 **Type Safety** - Fully typed API with comprehensive error handling
- 🔒 **Secure** - Uses secure HTTPS connections with proper authentication

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
poe_api_process = "0.1.0"
```

Or use cargo:

```bash
cargo add poe_api_process
```

### With Debug Logging

To enable debug logging, add the `trace` feature:

```toml
[dependencies]
poe_api_process = { version = "0.1.0", features = ["trace"] }
```

## Logging

The library supports configurable debug logging through the `tracing` crate. When the `trace` feature is enabled, you can control logging output using environment variables or programmatically.

### Environment-based Configuration

```rust
// Initialize logging with RUST_LOG environment variable
#[cfg(feature = "trace")]
poe_api_process::init_tracing();

// Set environment variable before running:
// RUST_LOG=debug cargo run
// RUST_LOG=poe_api_process=trace cargo run
```

### Programmatic Configuration

```rust
// Initialize with custom filter
#[cfg(feature = "trace")]
poe_api_process::init_tracing_with_filter("poe_api_process=debug,reqwest=info");
```

## Quick Start

```rust
use poe_api_process::{PoeClient, ChatRequest, ChatMessage, ChatEventType};
use futures_util::StreamExt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create a client with your bot and access key
    let client = PoeClient::new("Llama-4-Scout", "your_access_key");
    
    // Create a chat request
    let request = ChatRequest {
        version: "1.1".to_string(),
        r#type: "query".to_string(),
        query: vec![ChatMessage {
            role: "user".to_string(),
            content: "Hello, how are you?".to_string(),
            content_type: "text/markdown".to_string(),
            attachments: None,
        }],
        // Required fields (use unique IDs in production)
        user_id: "user123".to_string(),
        conversation_id: "conv123".to_string(),
        message_id: "msg123".to_string(),
        // Optional fields
        tools: None,
        tool_calls: None,
        tool_results: None,
        temperature: None,
        logit_bias: None,
        stop_sequences: None,
    };
    
    // Stream the response
    let mut stream = client.stream_request(request).await?;
    
    while let Some(response) = stream.next().await {
        match response {
            Ok(event) => match event.event {
                ChatEventType::Text => {
                    if let Some(data) = event.data {
                        if let crate::types::ChatResponseData::Text { text } = data {
                            print!("{}", text);
                        }
                    }
                },
                ChatEventType::Done => {
                    println!("\nConversation complete");
                    break;
                },
                _ => {}
            },
            Err(e) => eprintln!("Error: {}", e),
        }
    }
    
    Ok(())
}
```

## API Reference

### Client Creation

```rust
let client = PoeClient::new("bot_name", "access_key");
```

### Streaming Responses

The library provides real-time streaming of bot responses:

```rust
let mut stream = client.stream_request(request).await?;

while let Some(response) = stream.next().await {
    // Handle each streaming event
}
```

### Event Types

The library supports the following event types:

- `ChatEventType::Text` - Incremental text response
- `ChatEventType::ReplaceResponse` - Replace the entire response
- `ChatEventType::Json` - JSON data (including tool calls)
- `ChatEventType::File` - File attachments in response
- `ChatEventType::Done` - Conversation complete
- `ChatEventType::Error` - Error occurred

### Tool Calls (Function Calling)

Define available tools and handle tool calls:

```rust
use serde_json::json;
use poe_api_process::{ChatTool, FunctionDefinition, FunctionParameters};

// Define a tool
let weather_tool = ChatTool {
    r#type: "function".to_string(),
    function: FunctionDefinition {
        name: "get_weather".to_string(),
        description: "Get weather information for a location".to_string(),
        parameters: FunctionParameters {
            r#type: "object".to_string(),
            properties: json!({
                "location": {
                    "type": "string",
                    "description": "City name"
                }
            }),
            required: vec!["location".to_string()],
        },
    },
};

// Include in request
let request = ChatRequest {
    tools: Some(vec![weather_tool]),
    // ... other fields
};

// Handle tool calls in response
if let ChatEventType::Json = event.event {
    if let Some(ChatResponseData::ToolCalls(tool_calls)) = event.data {
        // Process tool calls and send results back
        let tool_results = vec![ChatToolResult {
            role: "tool".to_string(),
            tool_call_id: tool_calls[0].id.clone(),
            name: tool_calls[0].function.name.clone(),
            content: r#"{"temperature": 25, "condition": "sunny"}"#.to_string(),
        }];
        
        let result_stream = client.send_tool_results(
            request.clone(),
            tool_calls,
            tool_results
        ).await?;
    }
}
```

### File Uploads

Upload files and attach them to messages:

```rust
use poe_api_process::{Attachment, FileUploadRequest};

// Upload a single local file
let upload_result = client.upload_local_file("path/to/file.pdf", None).await?;

// Upload with specific MIME type
let upload_result = client.upload_local_file("path/to/file.pdf", Some("application/pdf")).await?;

// Upload a remote file
let remote_result = client.upload_remote_file("https://example.com/file.pdf").await?;

// Batch upload multiple files
let batch_results = client.upload_files_batch(vec![
    FileUploadRequest::LocalFile { 
        file: "path/to/file1.pdf".to_string(),
        mime_type: None 
    },
    FileUploadRequest::RemoteFile { 
        download_url: "https://example.com/file2.pdf".to_string() 
    },
]).await?;

// Attach to message
let message = ChatMessage {
    attachments: Some(vec![Attachment {
        url: upload_result.attachment_url,
        content_type: upload_result.mime_type,
    }]),
    // ... other fields
};
```

### Model Discovery

Get available models without authentication:

```rust
use poe_api_process::get_model_list;

let models = get_model_list(Some("en")).await?;
for model in models.data {
    println!("{}: {}", model.id, model.owned_by);
}
```

## Error Handling

The library provides comprehensive error handling with the `PoeError` enum:

```rust
use poe_api_process::PoeError;

match client.stream_request(request).await {
    Ok(stream) => { /* handle stream */ },
    Err(PoeError::RequestFailed(e)) => { /* HTTP error */ },
    Err(PoeError::BotError(msg)) => { /* Bot-specific error */ },
    Err(PoeError::FileNotFound(path)) => { /* File doesn't exist */ },
    // ... other error types
}
```

## Debugging

Enable detailed logging with the `trace` feature:

```toml
[dependencies]
poe_api_process = { version = "0.1.0", features = ["trace"] }
```

Then set the log level:

```bash
RUST_LOG=poe_api_process=debug cargo run
```

## Security

This library is built with security best practices in mind:

### Core Security Features
- **No Unsafe Code**: The entire codebase uses `#![deny(unsafe_code)]`
- **Memory Safe**: No `unwrap()` or `expect()` in library code (only in tests)
- **HTTPS Only**: All API calls use HTTPS with TLS/SSL verification enabled
- **Input Validation**: Bot names, access keys, URLs, and file paths are validated
- **Error Handling**: Custom error types that never expose sensitive information like API keys

### Security Guidelines for Users
1. **API Key Storage**
   - Store API keys in environment variables or secure vaults
   - Never commit API keys to version control
   - Use `.env` files for local development (add to `.gitignore`)

2. **File Operations**
   - Only upload files from trusted sources
   - Validate file paths and URLs before passing to the library
   - Use HTTPS URLs only for remote file uploads

3. **Dependencies**
   - Keep dependencies updated with `cargo update`
   - Run `cargo audit` regularly to check for vulnerabilities

## Testing

This library includes comprehensive tests that cover all functionality:

### Test Coverage
The test suite includes 8 integration tests that verify:
- **Streaming**: Basic chat requests and response streaming
- **Model Discovery**: Fetching available models via GraphQL
- **Tool Calling**: Function calling with proper accumulation
- **File Uploads**: Both local and remote file handling
- **Error Handling**: Malformed data and missing file scenarios
- **Content Parsing**: Proper event and data parsing

### Running Tests

```bash
# Prerequisites: Set your Poe API key
echo "POE_ACCESS_KEY=your_p-b_cookie_value" > .env

# Run all tests
cargo test --features trace

# Run with debug output
RUST_LOG=poe_api_process=debug cargo test --features trace -- --nocapture

# Run a specific test
cargo test test_stream_request --features trace -- --nocapture

# Run tests sequentially (recommended for API tests)
cargo test --features trace -- --test-threads=1
```

### Getting Your API Key
1. Go to [poe.com](https://poe.com)
2. Open Developer Tools (F12)
3. Go to Storage/Application → Cookies
4. Find the cookie named `p-b` - this is your access key

### Test Organization
All tests follow a consistent pattern:
1. **Setup**: Initialize environment and client
2. **Arrange**: Prepare the request
3. **Act**: Make the API call
4. **Assert**: Verify the results
5. **Cleanup**: Clean up resources if needed

**Note**: Tests make real API calls to Poe.com, so they require a valid API key and internet connection.

## Requirements

- Rust 1.70 or higher
- Tokio runtime
- Valid [Poe API access key](https://poe.com/api_key)

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request. For major changes, please open an issue first to discuss what you would like to change.

## Support

For issues and feature requests, please use the [GitHub issue tracker](https://github.com/mehmetbaykar/poe_api_process/issues).
