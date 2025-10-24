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

// Initialize logging, ensure it only runs once
static INIT: Once = Once::new();

fn setup() {
    // Initialize logging
    INIT.call_once(|| {
        let _ = env_logger::builder().is_test(true).try_init();
    });
    // Load environment variables
    dotenv().ok();
    debug!("Test environment setup completed");
}

fn get_access_key() -> String {
    match env::var("POE_ACCESS_KEY") {
        Ok(key) => {
            debug!("Successfully read POE_ACCESS_KEY environment variable");
            key
        }
        Err(_) => {
            warn!("Failed to read POE_ACCESS_KEY environment variable");
            panic!("Need to set POE_ACCESS_KEY in .env file");
        }
    }
}

#[test_log::test(tokio::test)]
async fn test_stream_request() {
    setup();
    let access_key = get_access_key();
    debug!("Creating PoeClient test instance");
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

    debug!("Sending stream request");
    let result = client.stream_request(request).await;

    match &result {
        Ok(_) => debug!("Stream request successful"),
        Err(e) => warn!("Stream request failed: {}", e),
    }

    assert!(result.is_ok(), "Creating stream request should succeed");

    if let Ok(mut stream) = result {
        let mut received_response = false;
        debug!("Starting to process response stream");

        while let Some(response) = stream.next().await {
            match response {
                Ok(event) => {
                    received_response = true;
                    debug!("Received event: {:?}", event);
                }
                Err(e) => {
                    warn!("Stream processing error: {}", e);
                    panic!("Stream processing error: {}", e);
                }
            }
        }

        assert!(received_response, "Should receive at least one response");
        debug!("Stream request test completed");
    }
}

#[test_log::test(tokio::test)]
async fn test_get_model_list() {
    setup();
    debug!("Starting to test getting model list");
    let result = get_model_list(Some("zh-Hant")).await;

    match &result {
        Ok(models) => debug!("Successfully got model list, total {} models", models.data.len()),
        Err(e) => warn!("Failed to get model list: {}", e),
    }

    match result {
        Ok(models) => {
            assert!(!models.data.is_empty(), "Model list should not be empty");
            debug!("Successfully got {} models", models.data.len());
            // Verify first model's basic information
            if let Some(first_model) = models.data.first() {
                assert!(!first_model.id.is_empty(), "Model ID should not be empty");
                assert_eq!(first_model.object, "model", "Model type should be 'model'");
                assert_eq!(first_model.owned_by, "poe", "Model owner should be 'poe'");
                debug!("First model information:");
                debug!("ID: {}", first_model.id);
                debug!("Type: {}", first_model.object);
                debug!("Owner: {}", first_model.owned_by);
            }
        }
        Err(e) => {
            warn!("Failed to get model list: {}", e);
            panic!("Failed to get model list: {}", e);
        }
    }

    debug!("Get model list test completed");
}

#[test_log::test(tokio::test)]
async fn test_stream_content_verification() {
    setup();
    let access_key = get_access_key();
    debug!("Creating PoeClient test instance");
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

    debug!("Sending stream request to verify content");
    let result = client.stream_request(request).await;

    match &result {
        Ok(_) => debug!("Stream request successful"),
        Err(e) => warn!("Stream request failed: {}", e),
    }

    assert!(result.is_ok(), "Creating stream request should succeed");

    if let Ok(mut stream) = result {
        let mut received_text_event = false;
        debug!("Starting to process response stream to verify content");

        while let Some(response) = stream.next().await {
            match response {
                Ok(event_response) => {
                    debug!("Received event response: {:?}", event_response);
                    match event_response.event {
                        ChatEventType::Text => {
                            received_text_event = true;
                            if let Some(ChatResponseData::Text { text }) = event_response.data {
                                debug!("Received Text event, content: '{}'", text);
                            }
                        }
                        ChatEventType::Error => {
                            if let Some(ChatResponseData::Error { text, allow_retry }) =
                                event_response.data
                            {
                                warn!(
                                    "Received Error event: error message: {}, retry allowed: {}",
                                    text, allow_retry
                                );
                                panic!("Stream processing received Error event: {}", text);
                            }
                        }
                        _ => {
                            debug!("Received other type of event: {:?}", event_response.event);
                        }
                    }
                }
                Err(e) => {
                    warn!("Stream processing error: {}", e);
                    panic!("Stream processing error: {}", e);
                }
            }
        }

        assert!(received_text_event, "Should receive at least one Event::Text event");
        debug!("Stream content verification test completed");
    }
}

#[test_log::test(tokio::test)]
async fn test_stream_tool_content_verification() {
    setup();
    let access_key = get_access_key();
    debug!("Creating PoeClient test instance for tool content testing");
    let client = PoeClient::new(
        "GPT-4o-Mini",
        &access_key,
        "https://api.poe.com",
        "https://www.quora.com/poe_api/file_upload_3RD_PARTY_POST",
    );

    // Create request with tool definitions
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

    debug!("Sending stream request with tool definitions");
    let result = client.stream_request(request).await;

    match &result {
        Ok(_) => debug!("Tool stream request successful"),
        Err(e) => warn!("Tool stream request failed: {}", e),
    }

    assert!(result.is_ok(), "Creating tool stream request should succeed");

    if let Ok(mut stream) = result {
        let mut received_tool_call = false;
        debug!("Starting to process tool response stream");

        while let Some(response) = stream.next().await {
            match response {
                Ok(event_response) => {
                    debug!("Received tool-related event: {:?}", event_response.event);

                    match event_response.event {
                        ChatEventType::Json => {
                            // Check if there are tool_calls
                            if let Some(ChatResponseData::ToolCalls(tool_calls)) =
                                event_response.data
                            {
                                received_tool_call = true;
                                debug!("Received ToolCalls event, tool call count: {}", tool_calls.len());

                                // Verify tool call content
                                for tool_call in tool_calls {
                                    assert_eq!(
                                        tool_call.r#type, "function",
                                        "Tool call type should be function"
                                    );
                                    assert!(!tool_call.id.is_empty(), "Tool call ID should not be empty");
                                    assert_eq!(
                                        tool_call.function.name, "get_weather",
                                        "Tool call function name should be get_weather"
                                    );
                                    debug!("Tool call parameters: {}", tool_call.function.arguments);
                                }

                                // Since we've confirmed receiving tool calls, we can choose to exit the loop
                                break;
                            } else {
                                debug!("Received Json event, but doesn't contain tool_calls, might be incremental update");
                            }
                        }
                        ChatEventType::Error => {
                            if let Some(ChatResponseData::Error { text, allow_retry }) =
                                event_response.data
                            {
                                warn!(
                                    "Received Error event: error message: {}, retry allowed: {}",
                                    text, allow_retry
                                );
                                panic!("Tool stream processing received Error event: {}", text);
                            }
                        }
                        _ => {
                            debug!("Received other type of event: {:?}", event_response.event);
                        }
                    }
                }
                Err(e) => {
                    warn!("Tool stream processing error: {}", e);
                    panic!("Tool stream processing error: {}", e);
                }
            }
        }

        // Note: Depending on model response, tool calls may not always occur
        // Therefore, we don't use strict assertions here, but record the result
        if received_tool_call {
            debug!("Successfully received tool call event");
        } else {
            debug!("No tool call event received, this might be normal depending on model response");
        }

        debug!("Tool-related stream test completed");
    }
}

#[test_log::test(tokio::test)]
async fn test_tool_calls_parsing() {
    setup();
    debug!("Starting tool call parsing test");

    // Simulated tool call JSON data
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

    // Parse tool calls
    let tool_calls_value = tool_calls_json.get("tool_calls").unwrap();
    let tool_calls: Vec<ChatToolCall> = serde_json::from_value(tool_calls_value.clone()).unwrap();

    // Verify parsing results
    assert_eq!(tool_calls.len(), 1, "Should parse one tool call");
    assert_eq!(tool_calls[0].id, "call_123456", "Tool call ID should match");
    assert_eq!(
        tool_calls[0].r#type, "function",
        "Tool call type should be function"
    );
    assert_eq!(
        tool_calls[0].function.name, "get_weather",
        "Tool call function name should be get_weather"
    );
    assert_eq!(
        tool_calls[0].function.arguments, "{\"location\":\"Taipei\",\"unit\":\"celsius\"}",
        "Tool call parameters should match"
    );

    debug!("Tool call parsing test completed");
}

#[test_log::test(tokio::test)]
async fn test_tool_call_parse_error() {
    setup();
    debug!("Starting tool call parse error handling test");

    // Simulated malformed tool call JSON data
    let invalid_tool_calls_json = json!({
        "tool_calls": [
            {
                "id": "call_123456",
                "type": "function",
                "function": {
                    "name": "get_weather",
                    // Missing arguments field, this will cause parsing error
                }
            }
        ]
    });

    // Try parsing invalid tool calls
    let tool_calls_value = invalid_tool_calls_json.get("tool_calls").unwrap();
    let parse_result: Result<Vec<ChatToolCall>, _> =
        serde_json::from_value(tool_calls_value.clone());

    // Verify parsing result should be error
    assert!(parse_result.is_err(), "Parsing invalid tool calls should fail");

    // Verify error type
    let error = parse_result.unwrap_err();
    debug!("Parsing error: {}", error);
    assert!(
        error.to_string().contains("missing field"),
        "Error message should indicate missing field"
    );

    debug!("Tool call parse error handling test completed");
}

#[test_log::test(tokio::test)]
async fn test_file_upload() {
    setup();
    let access_key = get_access_key();
    debug!("Creating PoeClient test instance for file upload testing");
    let client = PoeClient::new(
        "Claude-3.7-Sonnet",
        &access_key,
        "https://api.poe.com",
        "https://www.quora.com/poe_api/file_upload_3RD_PARTY_POST",
    );
    // Create a temporary file for testing
    use std::fs::File;
    use std::io::Write;
    use tempfile::tempdir;
    let temp_dir = tempdir().expect("Failed to create temporary directory");
    let file_path = temp_dir.path().join("test_upload.txt");
    let file_path_str = file_path.to_str().unwrap().to_string();
    debug!("Creating temporary test file: {}", file_path_str);
    {
        let mut file = File::create(&file_path).expect("Failed to create temporary file");
        writeln!(file, "This is test upload file content").expect("Failed to write to temporary file");
    }
    // Test local file upload
    debug!("Starting local file upload test");
    let upload_result = client.upload_local_file(&file_path_str, None).await;
    match &upload_result {
        Ok(response) => {
            debug!("File upload successful, attachment URL: {}", response.attachment_url);
            debug!("File MIME type: {}", response.mime_type.clone().unwrap());
            debug!("File size: {} bytes", response.size.unwrap());
        }
        Err(e) => warn!("File upload failed: {}", e),
    }
    assert!(upload_result.is_ok(), "Local file upload should succeed");
    if let Ok(response) = upload_result {
        assert!(!response.attachment_url.is_empty(), "Attachment URL should not be empty");
        assert_eq!(
            response.mime_type.unwrap(),
            "text/plain",
            "MIME type should be text/plain"
        );
        assert!(response.size.unwrap() > 0, "File size should be greater than 0");
    }
    // Test batch upload
    debug!("Starting batch upload test");
    let batch_upload_requests = vec![
        FileUploadRequest::LocalFile {
            file: file_path_str.clone(),
            mime_type: None,
        },
        // Can add remote file test, but need valid URL
        // FileUploadRequest::RemoteFile { download_url: "https://example.com/sample.txt".to_string() },
    ];
    let batch_result = client.upload_files_batch(batch_upload_requests).await;
    match &batch_result {
        Ok(responses) => debug!("Batch upload successful, total {} files", responses.len()),
        Err(e) => warn!("Batch upload failed: {}", e),
    }
    assert!(batch_result.is_ok(), "Batch upload should succeed");
    if let Ok(responses) = batch_result {
        assert!(!responses.is_empty(), "Should upload at least one file");
        assert!(
            !responses[0].attachment_url.is_empty(),
            "Batch upload attachment URL should not be empty"
        );
    }
    // Test message sending with attachments
    debug!("Starting message sending with attachments test");
    let file_upload_response = client
        .upload_local_file(&file_path_str, None)
        .await
        .expect("File upload failed");
    let request = ChatRequest {
        version: "1.1".to_string(),
        r#type: "query".to_string(),
        query: vec![ChatMessage {
            role: "user".to_string(),
            content: "This is a message with an attached file, please analyze the file content".to_string(),
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
    debug!("Sending message request with attachments");
    let result = client.stream_request(request).await;
    match &result {
        Ok(_) => debug!("Message request with attachments successful"),
        Err(e) => warn!("Message request with attachments failed: {}", e),
    }
    assert!(result.is_ok(), "Message request with attachments should succeed");
    if let Ok(mut stream) = result {
        let mut received_response = false;
        debug!("Starting to process message response stream with attachments");
        while let Some(response) = stream.next().await {
            match response {
                Ok(event) => {
                    received_response = true;
                    debug!("Received message event with attachments: {:?}", event);
                    // Check if response mentions attachments or files
                    if let Some(ChatResponseData::Text { text }) = &event.data {
                        if text.contains("file") || text.contains("content") {
                            debug!("Response mentions file or content, confirming attachment was processed");
                        }
                    }
                }
                Err(e) => {
                    warn!("Stream processing error for message with attachments: {}", e);
                    panic!("Stream processing error for message with attachments: {}", e);
                }
            }
        }
        assert!(received_response, "Should receive at least one response for message with attachments");
    }
    // Test invalid file path
    debug!("Starting invalid file path test");
    let invalid_path = "non-existent-file-path.txt";
    let invalid_result = client.upload_local_file(invalid_path, None).await;
    match &invalid_result {
        Ok(_) => warn!("Uploading non-existent file succeeded, this is unexpected"),
        Err(e) => debug!("As expected, uploading non-existent file failed: {}", e),
    }
    assert!(invalid_result.is_err(), "Uploading non-existent file should fail");
    // Clean up temporary files
    debug!("Test completed, cleaning up temporary files");
    temp_dir.close().expect("Failed to clean up temporary directory");
}

#[test_log::test(tokio::test)]
async fn test_remote_file_upload() {
    setup();
    let access_key = get_access_key();
    debug!("Creating PoeClient test instance for remote file upload testing");
    let client = PoeClient::new(
        "Claude-3.7-Sonnet",
        &access_key,
        "https://api.poe.com",
        "https://www.quora.com/poe_api/file_upload_3RD_PARTY_POST",
    );

    // Use publicly accessible test file URL
    let test_url = "https://www.w3.org/WAI/ER/tests/xhtml/testfiles/resources/pdf/dummy.pdf";

    debug!("Starting remote file upload test, URL: {}", test_url);
    let upload_result = client.upload_remote_file(test_url).await;
    match &upload_result {
        Ok(response) => {
            debug!("Remote file upload successful, attachment URL: {}", response.attachment_url);
            debug!("File MIME type: {}", response.mime_type.clone().unwrap());
            debug!("File size: {} bytes", response.size.unwrap());
        }
        Err(e) => warn!("Remote file upload failed: {}", e),
    }

    // Note: Since remote servers may be unreliable, we don't force assertion that this must succeed
    // But if successful, we check if response format is correct
    if let Ok(response) = upload_result {
        assert!(!response.attachment_url.is_empty(), "Attachment URL should not be empty");
        assert!(!response.mime_type.unwrap().is_empty(), "MIME type should not be empty");
        assert!(response.size.unwrap() > 0, "File size should be greater than 0");
        debug!("Remote file upload test completed");
    } else {
        debug!("Remote file upload failed, this might be network issue or service limitation");
    }

    // Test invalid URL
    debug!("Testing invalid URL");
    let invalid_url = "invalid-url";
    let invalid_result = client.upload_remote_file(invalid_url).await;
    match &invalid_result {
        Ok(_) => warn!("Uploading invalid URL succeeded, this is unexpected"),
        Err(e) => debug!("As expected, uploading invalid URL failed: {}", e),
    }
    assert!(invalid_result.is_err(), "Uploading invalid URL should fail");
}

#[test_log::test(tokio::test)]
async fn test_get_v1_model_list() {
    setup();
    let access_key = get_access_key();
    debug!("Starting v1/models model list test");

    let client = PoeClient::new(
        "Claude-3.7-Sonnet",
        &access_key,
        "https://api.poe.com",
        "https://www.quora.com/poe_api/file_upload_3RD_PARTY_POST",
    );
    let result = client.get_v1_model_list().await;

    match &result {
        Ok(models) => debug!(
            "Successfully got v1/models model list, total {} models",
            models.data.len()
        ),
        Err(e) => warn!("Failed to get v1/models model list: {}", e),
    }

    match result {
        Ok(models) => {
            assert!(!models.data.is_empty(), "v1/models model list should not be empty");
            debug!("Successfully got {} v1 models", models.data.len());

            // Verify first model's basic information
            if let Some(first_model) = models.data.first() {
                assert!(!first_model.id.is_empty(), "Model ID should not be empty");
                assert_eq!(first_model.object, "model", "Model type should be 'model'");
                assert_eq!(first_model.owned_by, "poe", "Model owner should be 'poe'");

                debug!("First v1 model information:");
                debug!("ID: {}", first_model.id);
                debug!("Type: {}", first_model.object);
                debug!("Owner: {}", first_model.owned_by);
                debug!("Created: {}", first_model.created);
            }
        }
        Err(e) => {
            warn!("Failed to get v1/models model list: {}", e);
            panic!("Failed to get v1/models model list: {}", e);
        }
    }

    debug!("Get v1/models model list test completed");
}

// XML parsing test cases
#[cfg(feature = "xml")]
#[test_log::test(tokio::test)]
async fn test_xml_tool_call_detection() {
    setup();
    debug!("Starting XML tool call detection test");

    let message = ChatMessage {
        role: "assistant".to_string(),
        content: "I need to query weather information.\n\n<tool_call>\n<invoke name=\"get_weather\">\n<parameter name=\"location\">Taipei</parameter>\n</invoke>\n</tool_call>\n\nPlease wait a moment.".to_string(),
        attachments: None,
        content_type: "text/plain".to_string(),
    };

    assert!(message.contains_xml_tool_calls(), "Should detect XML tool calls");
    debug!("XML tool call detection test completed");
}

#[cfg(feature = "xml")]
#[test_log::test(tokio::test)]
async fn test_xml_tool_call_extraction() {
    setup();
    debug!("Starting XML tool call extraction test");

    let message = ChatMessage {
        role: "assistant".to_string(),
        content: "I'll help you query the weather.\n\n<tool_call>\n<invoke name=\"get_weather\">\n<parameter name=\"location\">Taipei</parameter>\n<parameter name=\"unit\">celsius</parameter>\n</invoke>\n</tool_call>\n\nQuerying...".to_string(),
        attachments: None,
        content_type: "text/plain".to_string(),
    };

    let tool_calls = message.extract_xml_tool_calls();

    assert_eq!(tool_calls.len(), 1, "Should extract one tool call");
    assert_eq!(
        tool_calls[0].function.name, "get_weather",
        "Tool name should be get_weather"
    );

    // Parse parameters
    let args: serde_json::Value =
        serde_json::from_str(&tool_calls[0].function.arguments).expect("Parameters should be valid JSON");
    assert_eq!(args["location"], "Taipei", "location parameter should be Taipei");
    assert_eq!(args["unit"], "celsius", "unit parameter should be celsius");

    debug!("XML tool call extraction test completed");
}

#[cfg(feature = "xml")]
#[test_log::test(tokio::test)]
async fn test_multiple_xml_tool_calls() {
    setup();
    debug!("Starting multiple XML tool calls test");

    let message = ChatMessage {
        role: "assistant".to_string(),
        content: "I need to perform two operations:\n\n<tool_call>\n<invoke name=\"get_weather\">\n<parameter name=\"location\">Taipei</parameter>\n</invoke>\n</tool_call>\n\n<tool_call>\n<invoke name=\"calculate\">\n<parameter name=\"expression\">2+2</parameter>\n</invoke>\n</tool_call>\n\nPlease wait.".to_string(),
        attachments: None,
        content_type: "text/plain".to_string(),
    };

    let tool_calls = message.extract_xml_tool_calls();

    assert_eq!(tool_calls.len(), 2, "Should extract two tool calls");
    assert_eq!(
        tool_calls[0].function.name, "get_weather",
        "First tool should be get_weather"
    );
    assert_eq!(
        tool_calls[1].function.name, "calculate",
        "Second tool should be calculate"
    );

    // Check first tool call parameters
    let args1: serde_json::Value = serde_json::from_str(&tool_calls[0].function.arguments)
        .expect("First tool parameters should be valid JSON");
    assert_eq!(
        args1["location"], "Taipei",
        "First tool location parameter should be Taipei"
    );

    // Check second tool call parameters
    let args2: serde_json::Value = serde_json::from_str(&tool_calls[1].function.arguments)
        .expect("Second tool parameters should be valid JSON");
    assert_eq!(
        args2["expression"], "2+2",
        "Second tool expression parameter should be 2+2"
    );

    debug!("Multiple XML tool calls test completed");
}

#[cfg(feature = "xml")]
#[test_log::test(tokio::test)]
async fn test_xml_tool_call_with_complex_parameters() {
    setup();
    debug!("Starting XML tool call with complex parameters test");

    let message = ChatMessage {
        role: "assistant".to_string(),
        content: "<tool_call>\n<invoke name=\"send_email\">\n<parameter name=\"to\">user@example.com</parameter>\n<parameter name=\"subject\">Test Email</parameter>\n<parameter name=\"body\">This is a test email with special characters: &lt;test&gt;</parameter>\n<parameter name=\"priority\">high</parameter>\n</invoke>\n</tool_call>".to_string(),
        attachments: None,
        content_type: "text/plain".to_string(),
    };

    let tool_calls = message.extract_xml_tool_calls();

    assert_eq!(tool_calls.len(), 1, "Should extract one tool call");
    assert_eq!(
        tool_calls[0].function.name, "send_email",
        "Tool name should be send_email"
    );

    let args: serde_json::Value =
        serde_json::from_str(&tool_calls[0].function.arguments).expect("Parameters should be valid JSON");
    assert_eq!(args["to"], "user@example.com", "to parameter should be correct");
    assert_eq!(args["subject"], "Test Email", "subject parameter should be correct");
    assert_eq!(
        args["body"], "This is a test email with special characters: <test>",
        "body parameter should correctly decode XML entities"
    );
    assert_eq!(args["priority"], "high", "priority parameter should be correct");

    debug!("XML tool call with complex parameters test completed");
}

#[cfg(feature = "xml")]
#[test_log::test(tokio::test)]
async fn test_no_xml_tool_calls() {
    setup();
    debug!("Starting no XML tool calls test");

    let message = ChatMessage {
        role: "assistant".to_string(),
        content: "This is a normal response without tool calls.".to_string(),
        attachments: None,
        content_type: "text/plain".to_string(),
    };

    assert!(
        !message.contains_xml_tool_calls(),
        "Should not detect XML tool calls"
    );
    let tool_calls = message.extract_xml_tool_calls();
    assert!(tool_calls.is_empty(), "Should not extract any tool calls");

    debug!("No XML tool calls test completed");
}

#[cfg(feature = "xml")]
#[test_log::test(tokio::test)]
async fn test_xml_tool_call_with_empty_parameters() {
    setup();
    debug!("Starting XML tool call with empty parameters test");

    let message = ChatMessage {
        role: "assistant".to_string(),
        content:
            "Execute parameterless tool.\n\n<tool_call>\n<invoke name=\"get_time\">\n</invoke>\n</tool_call>"
                .to_string(),
        attachments: None,
        content_type: "text/plain".to_string(),
    };

    let tool_calls = message.extract_xml_tool_calls();

    assert_eq!(tool_calls.len(), 1, "Should extract one tool call");
    assert_eq!(
        tool_calls[0].function.name, "get_time",
        "Tool name should be get_time"
    );

    let args: serde_json::Value =
        serde_json::from_str(&tool_calls[0].function.arguments).expect("Parameters should be valid JSON");
    assert!(args.is_object(), "Parameters should be an empty object");
    assert_eq!(args.as_object().unwrap().len(), 0, "Parameter object should be empty");

    debug!("XML tool call with empty parameters test completed");
}

#[cfg(feature = "xml")]
#[test_log::test(tokio::test)]
async fn test_xml_tool_call_parsing_error_handling() {
    setup();
    debug!("Starting XML tool call parsing error handling test");

    // Test malformed XML
    let message_with_invalid_xml = ChatMessage {
        role: "assistant".to_string(),
        content: "Malformed XML.\n\n<tool_call>\n<invoke name=\"get_weather\">\n<parameter name=\"location\">Taipei\n</invoke>\n</tool_call>".to_string(),
        attachments: None,
        content_type: "text/plain".to_string(),
    };

    // Even if XML format has issues, function should handle without crashing
    let tool_calls = message_with_invalid_xml.extract_xml_tool_calls();
    // Due to XML format errors, may not parse correctly, but should not crash
    debug!("Malformed XML parsing result: {} tool calls", tool_calls.len());

    debug!("XML tool call parsing error handling test completed");
}

#[cfg(feature = "xml")]
#[test_log::test(tokio::test)]
async fn test_xml_entity_decoding() {
    setup();
    debug!("Starting XML entity decoding test");

    let message = ChatMessage {
        role: "assistant".to_string(),
        content: "<tool_call>\n<invoke name=\"test_tool\">\n<parameter name=\"text\">&lt;hello&gt; &amp; &quot;world&quot; &apos;test&apos;</parameter>\n</invoke>\n</tool_call>".to_string(),
        attachments: None,
        content_type: "text/plain".to_string(),
    };

    let tool_calls = message.extract_xml_tool_calls();

    assert_eq!(tool_calls.len(), 1, "Should extract one tool call");

    let args: serde_json::Value =
        serde_json::from_str(&tool_calls[0].function.arguments).expect("Parameters should be valid JSON");
    assert_eq!(
        args["text"], "<hello> & \"world\" 'test'",
        "XML entities should be correctly decoded"
    );

    debug!("XML entity decoding test completed");
}

#[cfg(feature = "xml")]
#[test_log::test(tokio::test)]
async fn test_dynamic_xml_tool_call_detection() {
    setup();
    debug!("Starting dynamic XML tool call detection test");

    // Create custom tool definitions
    let custom_tools = vec![
        ChatTool {
            r#type: "function".to_string(),
            function: FunctionDefinition {
                name: "custom_weather_api".to_string(),
                description: Some("Custom weather API".to_string()),
                parameters: Some(FunctionParameters {
                    r#type: "object".to_string(),
                    properties: json!({
                        "city": {
                            "type": "string",
                            "description": "City name"
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
                description: Some("Send notification".to_string()),
                parameters: Some(FunctionParameters {
                    r#type: "object".to_string(),
                    properties: json!({
                        "message": {
                            "type": "string",
                            "description": "Notification message"
                        }
                    }),
                    required: vec!["message".to_string()],
                }),
            },
        },
    ];

    // Test message with custom tool tags
    let message_with_custom_tool = ChatMessage {
        role: "assistant".to_string(),
        content: "I need to query weather.\n\n<custom_weather_api>\n<city>Taipei</city>\n</custom_weather_api>\n\nQuerying...".to_string(),
        attachments: None,
        content_type: "text/plain".to_string(),
    };

    // Use tool definition-based detection
    assert!(
        message_with_custom_tool.contains_xml_tool_calls_with_tools(&custom_tools),
        "Should detect custom tool calls"
    );

    // Test message without any tool tags
    let message_without_tools = ChatMessage {
        role: "assistant".to_string(),
        content: "This is a normal response without any tool calls.".to_string(),
        attachments: None,
        content_type: "text/plain".to_string(),
    };

    assert!(
        !message_without_tools.contains_xml_tool_calls_with_tools(&custom_tools),
        "Should not detect tool calls"
    );

    debug!("Dynamic XML tool call detection test completed");
}

#[cfg(feature = "xml")]
#[test_log::test(tokio::test)]
async fn test_dynamic_xml_tool_call_extraction() {
    setup();
    debug!("Starting dynamic XML tool call extraction test");

    // Create custom tool definitions
    let custom_tools = vec![ChatTool {
        r#type: "function".to_string(),
        function: FunctionDefinition {
            name: "database_query".to_string(),
            description: Some("Database query".to_string()),
            parameters: Some(FunctionParameters {
                r#type: "object".to_string(),
                properties: json!({
                    "table": {
                        "type": "string",
                        "description": "Table name"
                    },
                    "conditions": {
                        "type": "string",
                        "description": "Query conditions"
                    }
                }),
                required: vec!["table".to_string()],
            }),
        },
    }];

    // Test message with custom tool calls
    let message = ChatMessage {
        role: "assistant".to_string(),
        content: "I need to query the database.\n\n<database_query>\n<table>users</table>\n<conditions>age > 18</conditions>\n</database_query>\n\nQuerying...".to_string(),
        attachments: None,
        content_type: "text/plain".to_string(),
    };

    debug!("Test message content: {}", message.content);
    debug!(
        "Contains database_query tag: {}",
        message.content.contains("<database_query>")
    );

    // First test general method
    let general_tool_calls = message.extract_xml_tool_calls();
    debug!("General method extracted tool calls count: {}", general_tool_calls.len());

    // Then test tool definition-based method
    let tool_calls = message.extract_xml_tool_calls_with_tools(&custom_tools);
    debug!("Tool definition-based extracted tool calls count: {}", tool_calls.len());

    if !tool_calls.is_empty() {
        debug!("Tool call content: {:?}", tool_calls[0]);
        debug!("Parameter string: {}", tool_calls[0].function.arguments);

        // Parse parameters
        let args: serde_json::Value =
            serde_json::from_str(&tool_calls[0].function.arguments).expect("Parameters should be valid JSON");
        debug!("Parsed parameters: {:?}", args);

        assert_eq!(
            tool_calls[0].function.name, "database_query",
            "Tool name should be database_query"
        );
        assert_eq!(args["table"], "users", "table parameter should be users");
        assert_eq!(
            args["conditions"], "age > 18",
            "conditions parameter should be age > 18"
        );
    } else {
        debug!("No tool calls extracted");
        panic!("Should extract one tool call");
    }

    debug!("Dynamic XML tool call extraction test completed");
}

#[cfg(feature = "xml")]
#[test_log::test(tokio::test)]
async fn test_potential_tool_name_detection() {
    setup();
    debug!("Starting potential tool name detection test");

    // Create tool definitions containing fetch_data tool
    let tools_with_fetch_data = vec![ChatTool {
        r#type: "function".to_string(),
        function: FunctionDefinition {
            name: "fetch_data".to_string(),
            description: Some("Fetch data".to_string()),
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

    // Test message with potential tool name
    let message_with_potential_tool = ChatMessage {
        role: "assistant".to_string(),
        content: "I need to perform operation.\n\n<fetch_data>\n<url>https://api.example.com</url>\n</fetch_data>\n\nProcessing...".to_string(),
        attachments: None,
        content_type: "text/plain".to_string(),
    };

    assert!(
        message_with_potential_tool.contains_xml_tool_calls_with_tools(&tools_with_fetch_data),
        "Should detect potential tool call (fetch_data)"
    );

    // Test message with HTML tags (should not be detected as tool calls)
    let message_with_html = ChatMessage {
        role: "assistant".to_string(),
        content: "This is a response containing HTML:\n\n<div>\n<p>This is a paragraph</p>\n</div>".to_string(),
        attachments: None,
        content_type: "text/plain".to_string(),
    };

    assert!(
        !message_with_html.contains_xml_tool_calls_with_tools(&tools_with_fetch_data),
        "Should not detect HTML tags as tool calls"
    );

    // Create tool definitions containing getUserData tool
    let tools_with_get_user_data = vec![ChatTool {
        r#type: "function".to_string(),
        function: FunctionDefinition {
            name: "getUserData".to_string(),
            description: Some("Get user data".to_string()),
            parameters: Some(FunctionParameters {
                r#type: "object".to_string(),
                properties: json!({
                    "userId": {
                        "type": "string",
                        "description": "User ID"
                    }
                }),
                required: vec!["userId".to_string()],
            }),
        },
    }];

    // Test message with camelCase tool
    let message_with_camel_case = ChatMessage {
        role: "assistant".to_string(),
        content: "Execute operation.\n\n<getUserData>\n<userId>123</userId>\n</getUserData>".to_string(),
        attachments: None,
        content_type: "text/plain".to_string(),
    };

    assert!(
        message_with_camel_case.contains_xml_tool_calls_with_tools(&tools_with_get_user_data),
        "Should detect camelCase tool call (getUserData)"
    );

    debug!("Potential tool name detection test completed");
}

#[cfg(feature = "xml")]
#[test_log::test(tokio::test)]
async fn test_mixed_tool_call_formats() {
    setup();
    debug!("Starting mixed tool call formats test");

    // Create tool definitions with multiple formats
    let tools = vec![ChatTool {
        r#type: "function".to_string(),
        function: FunctionDefinition {
            name: "standard_tool".to_string(),
            description: Some("Standard tool".to_string()),
            parameters: Some(FunctionParameters {
                r#type: "object".to_string(),
                properties: json!({
                    "param": {
                        "type": "string",
                        "description": "Parameter"
                    }
                }),
                required: vec!["param".to_string()],
            }),
        },
    }];

    // Test message with multiple formats
    let message = ChatMessage {
        role: "assistant".to_string(),
        content: r#"I need to perform multiple operations:

1. Standard format:
<tool_call>
<invoke name="standard_tool">
<parameter name="param">value1</parameter>
</invoke>
</tool_call>

2. Simplified format:
<standard_tool>
<param>value2</param>
</standard_tool>

Processing..."#
            .to_string(),
        attachments: None,
        content_type: "text/plain".to_string(),
    };

    let tool_calls = message.extract_xml_tool_calls_with_tools(&tools);

    // Should be able to parse both formats of tool calls
    assert!(tool_calls.len() >= 1, "Should extract at least one tool call");

    // Check if contains standard tool
    let has_standard_tool = tool_calls
        .iter()
        .any(|call| call.function.name == "standard_tool");
    assert!(has_standard_tool, "Should contain standard_tool call");

    debug!("Mixed tool call formats test completed");
}

#[cfg(feature = "xml")]
#[test_log::test(tokio::test)]
async fn test_remove_xml_tool_calls_with_tool_cells() {
    setup();
    debug!("Starting test for removing XML with tool calls");

    use crate::client::PoeClient;

    // Test text containing tool calls
    let text_with_tools = r#"I need to query weather information.

<tool_call>
<invoke name="get_weather">
<parameter name="location">Taipei</parameter>
<parameter name="unit">celsius</parameter>
</invoke>
</tool_call>

Please wait a moment, I'm querying the weather for Taipei."#;

    let cleaned_text = PoeClient::remove_xml_tool_calls(text_with_tools);

    // Should remove tool call part
    assert!(
        !cleaned_text.contains("<tool_call>"),
        "Should remove tool_call tags"
    );
    assert!(!cleaned_text.contains("<invoke"), "Should remove invoke tags");
    assert!(
        !cleaned_text.contains("<parameter"),
        "Should remove parameter tags"
    );
    assert!(
        cleaned_text.contains("I need to query weather information."),
        "Should preserve normal text"
    );
    assert!(
        cleaned_text.contains("Please wait a moment, I'm querying the weather for Taipei."),
        "Should preserve normal text"
    );

    debug!("XML tool call removal test completed");
}

#[cfg(feature = "xml")]
#[test_log::test(tokio::test)]
async fn test_remove_xml_tool_calls_without_tool_cells() {
    setup();
    debug!("Starting test for removing XML without tool calls");

    use crate::client::PoeClient;

    // Test text without tool calls
    let text_without_tools = r#"This is a normal response without any tool calls.
I can provide general help and information."#;

    let cleaned_text = PoeClient::remove_xml_tool_calls(text_without_tools);

    // Should keep original text unchanged
    assert_eq!(
        cleaned_text, text_without_tools,
        "Text without tool calls should remain unchanged"
    );

    debug!("XML removal without tool calls test completed");
}
