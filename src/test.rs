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

// Initialize logging, ensuring it only executes once
static INIT: Once = Once::new();

fn setup() {
    // Initialize logging
    INIT.call_once(|| {
        let _ = env_logger::builder().is_test(true).try_init();
    });
    // Load environment variables
    dotenv().ok();
    debug!("Test environment setup complete");
}

fn get_access_key() -> String {
    match env::var("POE_ACCESS_KEY") {
        Ok(key) => {
            debug!("Successfully read POE_ACCESS_KEY environment variable");
            key
        }
        Err(_) => {
            warn!("Could not read POE_ACCESS_KEY environment variable");
            panic!("POE_ACCESS_KEY must be set in .env file");
        }
    }
}

#[test_log::test(tokio::test)]
async fn test_stream_request() {
    setup();
    let access_key = get_access_key();
    debug!("Create PoeClient test instance");
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

    assert!(result.is_ok(), "Stream request should be successful");

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
                    warn!("Error processing stream: {}", e);
                    panic!("Error processing stream: {}", e);
                }
            }
        }

        assert!(received_response, "Should receive at least one response");
        debug!("Stream request test complete");
    }
}

#[test_log::test(tokio::test)]
async fn test_get_model_list() {
    setup();
    debug!("Starting test to get model list");
    let result = get_model_list(Some("zh-Hant")).await;

    match &result {
        Ok(models) => debug!("Successfully retrieved model list, {} models", models.data.len()),
        Err(e) => warn!("Failed to get model list: {}", e),
    }

    match result {
        Ok(models) => {
            assert!(!models.data.is_empty(), "Model list should not be empty");
            debug!("Successfully retrieved {} models", models.data.len());
            // Verify basic info of the first model
            if let Some(first_model) = models.data.first() {
                assert!(!first_model.id.is_empty(), "Model ID should not be empty");
                assert_eq!(first_model.object, "model", "Model type should be 'model'");
                assert_eq!(first_model.owned_by, "poe", "Model owner should be 'poe'");
                debug!("First model info:");
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

    debug!("Get model list test complete");
}

#[test_log::test(tokio::test)]
async fn test_stream_content_verification() {
    setup();
    let access_key = get_access_key();
    debug!("Create PoeClient test instance");
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

    assert!(result.is_ok(), "Stream request should be successful");

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
                                    "Received Error event: error message: {}, retryable: {}",
                                    text, allow_retry
                                );
                                panic!("Stream processing received Error event: {}", text);
                            }
                        }
                        _ => {
                            debug!("Received other event type: {:?}", event_response.event);
                        }
                    }
                }
                Err(e) => {
                    warn!("Error processing stream: {}", e);
                    panic!("Error processing stream: {}", e);
                }
            }
        }

        assert!(received_text_event, "Should receive at least one Event::Text event");
        debug!("Stream content verification test complete");
    }
}

#[test_log::test(tokio::test)]
async fn test_stream_tool_content_verification() {
    setup();
    let access_key = get_access_key();
    debug!("Create PoeClient test instance for tool content test");
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

    assert!(result.is_ok(), "Tool stream request should be successful");

    if let Ok(mut stream) = result {
        let mut received_tool_call = false;
        debug!("Starting to process tool response stream");

        while let Some(response) = stream.next().await {
            match response {
                Ok(event_response) => {
                    debug!("Received tool-related event: {:?}", event_response.event);

                    match event_response.event {
                        ChatEventType::Json => {
                            // Check if tool_calls are present
                            if let Some(ChatResponseData::ToolCalls(tool_calls)) =
                                event_response.data
                            {
                                received_tool_call = true;
                                debug!("Received ToolCalls event, number of tool calls: {}", tool_calls.len());

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

                                // Since we have confirmed tool calls, we can choose to exit the loop
                                break;
                            } else {
                                debug!("Received Json event, but no tool_calls, possibly incremental update");
                            }
                        }
                        ChatEventType::Error => {
                            if let Some(ChatResponseData::Error { text, allow_retry }) =
                                event_response.data
                            {
                                warn!(
                                    "Received Error event: error message: {}, retryable: {}",
                                    text, allow_retry
                                );
                                panic!("Tool stream processing received Error event: {}", text);
                            }
                        }
                        _ => {
                            debug!("Received other event type: {:?}", event_response.event);
                        }
                    }
                }
                Err(e) => {
                    warn!("Error processing tool stream: {}", e);
                    panic!("Error processing tool stream: {}", e);
                }
            }
        }

        // Note: The response of the model determines whether tool calls always occur
        // Therefore, we do not use strict assertions here, but record the result
        if received_tool_call {
            debug!("Successfully received tool call event");
        } else {
            debug!("No tool call event received, this may be normal, depending on model response");
        }

        debug!("Tool-related stream test complete");
    }
}

#[test_log::test(tokio::test)]
async fn test_tool_calls_parsing() {
    setup();
    debug!("Starting test for tool call parsing");

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

    // Verify parsing result
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

    debug!("Tool call parsing test complete");
}

#[test_log::test(tokio::test)]
async fn test_tool_call_parse_error() {
    setup();
    debug!("Starting test for tool call parsing error handling");

    // Simulated tool call JSON data with incorrect format
    let invalid_tool_calls_json = json!({
        "tool_calls": [
            {
                "id": "call_123456",
                "type": "function",
                "function": {
                    "name": "get_weather",
                    // Missing arguments field, this will cause a parsing error
                }
            }
        ]
    });

    // Attempt to parse invalid tool calls
    let tool_calls_value = invalid_tool_calls_json.get("tool_calls").unwrap();
    let parse_result: Result<Vec<ChatToolCall>, _> =
        serde_json::from_value(tool_calls_value.clone());

    // Verify parsing result should be an error
    assert!(parse_result.is_err(), "Parsing invalid tool calls should fail");

    // Verify error type
    let error = parse_result.unwrap_err();
    debug!("Parsing error: {}", error);
    assert!(
        error.to_string().contains("missing field"),
        "Error message should indicate missing field"
    );

    debug!("Tool call parsing error handling test complete");
}

#[test_log::test(tokio::test)]
async fn test_file_upload() {
    setup();
    let access_key = get_access_key();
    debug!("Create PoeClient test instance for file upload test");
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
    let temp_dir = tempdir().expect("Could not create temporary directory");
    let file_path = temp_dir.path().join("test_upload.txt");
    let file_path_str = file_path.to_str().unwrap().to_string();
    debug!("Creating temporary test file: {}", file_path_str);
    {
        let mut file = File::create(&file_path).expect("Could not create temporary file");
        writeln!(file, "This is a test file content for upload").expect("Could not write to temporary file");
    }
    // Test local file upload
    debug!("Starting test for local file upload");
    let upload_result = client.upload_local_file(&file_path_str, None).await;
    match &upload_result {
        Ok(response) => {
            debug!("File upload successful, attachment URL: {}", response.attachment_url);
            debug!("File MIME type: {}", response.mime_type.clone().unwrap());
            debug!("File size: {} bytes", response.size.unwrap());
        }
        Err(e) => warn!("File upload failed: {}", e),
    }
    assert!(upload_result.is_ok(), "Local file upload should be successful");
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
    debug!("Starting test for batch file upload");
    let batch_upload_requests = vec![
        FileUploadRequest::LocalFile {
            file: file_path_str.clone(),
            mime_type: None,
        },
        // Can add remote file test, but requires a valid URL
        // FileUploadRequest::RemoteFile { download_url: "https://example.com/sample.txt".to_string() },
    ];
    let batch_result = client.upload_files_batch(batch_upload_requests).await;
    match &batch_result {
        Ok(responses) => debug!("Batch upload successful, {} files", responses.len()),
        Err(e) => warn!("Batch upload failed: {}", e),
    }
    assert!(batch_result.is_ok(), "Batch upload should be successful");
    if let Ok(responses) = batch_result {
        assert!(!responses.is_empty(), "Should upload at least one file");
        assert!(
            !responses[0].attachment_url.is_empty(),
            "Attachment URL of batch upload should not be empty"
        );
    }
    // Test message with attachment
    debug!("Starting test for message with attachment");
    let file_upload_response = client
        .upload_local_file(&file_path_str, None)
        .await
        .expect("File upload failed");
    let request = ChatRequest {
        version: "1.1".to_string(),
        r#type: "query".to_string(),
        query: vec![ChatMessage {
            role: "user".to_string(),
            content: "This is a message with an attachment, please analyze the file content".to_string(),
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
    debug!("Sending message request with attachment");
    let result = client.stream_request(request).await;
    match &result {
        Ok(_) => debug!("Message request with attachment successful"),
        Err(e) => warn!("Message request with attachment failed: {}", e),
    }
    assert!(result.is_ok(), "Message request with attachment should be successful");
    if let Ok(mut stream) = result {
        let mut received_response = false;
        debug!("Starting to process message response stream with attachment");
        while let Some(response) = stream.next().await {
            match response {
                Ok(event) => {
                    received_response = true;
                    debug!("Received message event with attachment: {:?}", event);
                    // Check if the response mentioned attachment or file
                    if let Some(ChatResponseData::Text { text }) = &event.data {
                        if text.contains("file") || text.contains("content") {
                            debug!("Response mentioned file or content, confirming attachment handled");
                        }
                    }
                }
                Err(e) => {
                    warn!("Error processing message stream with attachment: {}", e);
                    panic!("Error processing message stream with attachment: {}", e);
                }
            }
        }
        assert!(received_response, "Should receive at least one message response with attachment");
    }
    // Test invalid file path
    debug!("Starting test for invalid file path");
    let invalid_path = "non_existent_file_path.txt";
    let invalid_result = client.upload_local_file(invalid_path, None).await;
    match &invalid_result {
        Ok(_) => warn!("Uploading non-existent file succeeded, which is unexpected"),
        Err(e) => debug!("As expected, uploading non-existent file failed: {}", e),
    }
    assert!(invalid_result.is_err(), "Uploading non-existent file should fail");
    // Clean up temporary file
    debug!("Test complete, cleaning up temporary file");
    temp_dir.close().expect("Could not clean up temporary directory");
}

#[test_log::test(tokio::test)]
async fn test_remote_file_upload() {
    setup();
    let access_key = get_access_key();
    debug!("Create PoeClient test instance for remote file upload test");
    let client = PoeClient::new(
        "Claude-3.7-Sonnet",
        &access_key,
        "https://api.poe.com",
        "https://www.quora.com/poe_api/file_upload_3RD_PARTY_POST",
    );

    // Use a publicly accessible test file URL
    let test_url = "https://www.w3.org/WAI/ER/tests/xhtml/testfiles/resources/pdf/dummy.pdf";

    debug!("Starting test for remote file upload, URL: {}", test_url);
    let upload_result = client.upload_remote_file(test_url).await;
    match &upload_result {
        Ok(response) => {
            debug!("Remote file upload successful, attachment URL: {}", response.attachment_url);
            debug!("File MIME type: {}", response.mime_type.clone().unwrap());
            debug!("File size: {} bytes", response.size.unwrap());
        }
        Err(e) => warn!("Remote file upload failed: {}", e),
    }

    // Note: Since remote services may be unreliable, we do not force assertions that it must succeed
    // If it succeeds, we check if the response format is correct
    if let Ok(response) = upload_result {
        assert!(!response.attachment_url.is_empty(), "Attachment URL should not be empty");
        assert!(!response.mime_type.unwrap().is_empty(), "MIME type should not be empty");
        assert!(response.size.unwrap() > 0, "File size should be greater than 0");
        debug!("Remote file upload test complete");
    } else {
        debug!("Remote file upload failed, this may be a network issue or service limitation");
    }

    // Test invalid URL
    debug!("Testing invalid URL");
    let invalid_url = "invalid-url";
    let invalid_result = client.upload_remote_file(invalid_url).await;
    match &invalid_result {
        Ok(_) => warn!("Uploading invalid URL succeeded, which is unexpected"),
        Err(e) => debug!("As expected, uploading invalid URL failed: {}", e),
    }
    assert!(invalid_result.is_err(), "Uploading invalid URL should fail");
}

#[test_log::test(tokio::test)]
async fn test_get_v1_model_list() {
    setup();
    let access_key = get_access_key();
    debug!("Starting test to get v1/models model list");

    let client = PoeClient::new(
        "Claude-3.7-Sonnet",
        &access_key,
        "https://api.poe.com",
        "https://www.quora.com/poe_api/file_upload_3RD_PARTY_POST",
    );
    let result = client.get_v1_model_list().await;

    match &result {
        Ok(models) => debug!(
            "Successfully retrieved v1/models model list, {} models",
            models.data.len()
        ),
        Err(e) => warn!("Failed to get v1/models model list: {}", e),
    }

    match result {
        Ok(models) => {
            assert!(!models.data.is_empty(), "v1/models model list should not be empty");
            debug!("Successfully retrieved {} v1 models", models.data.len());

            // Verify basic info of the first model
            if let Some(first_model) = models.data.first() {
                assert!(!first_model.id.is_empty(), "Model ID should not be empty");
                assert_eq!(first_model.object, "model", "Model type should be 'model'");
                assert_eq!(first_model.owned_by, "Poe", "Model owner should be 'Poe'");

                debug!("First v1 model info:");
                debug!("ID: {}", first_model.id);
                debug!("Type: {}", first_model.object);
                debug!("Owner: {}", first_model.owned_by);
                debug!("Created at: {}", first_model.created);
            }
        }
        Err(e) => {
            warn!("Failed to get v1/models model list: {}", e);
            panic!("Failed to get v1/models model list: {}", e);
        }
    }

    debug!("Get v1/models model list test complete");
}

// XML parsing test cases
#[cfg(feature = "xml")]
#[test_log::test(tokio::test)]
async fn test_xml_tool_call_detection() {
    setup();
    debug!("Starting test for XML tool call detection");

    let message = ChatMessage {
        role: "assistant".to_string(),
        content: "I need to query weather information.\n\n<tool_call>\n<invoke name=\"get_weather\">\n<parameter name=\"location\">Taipei</parameter>\n</invoke>\n</tool_call>\n\nPlease wait a moment.".to_string(),
        attachments: None,
        content_type: "text/plain".to_string(),
    };

    assert!(message.contains_xml_tool_calls(), "Should detect XML tool call");
    debug!("XML tool call detection test complete");
}

#[cfg(feature = "xml")]
#[test_log::test(tokio::test)]
async fn test_xml_tool_call_extraction() {
    setup();
    debug!("Starting test for XML tool call extraction");

    let message = ChatMessage {
        role: "assistant".to_string(),
        content: "I will help you query the weather.\n\n<tool_call>\n<invoke name=\"get_weather\">\n<parameter name=\"location\">Taipei</parameter>\n<parameter name=\"unit\">celsius</parameter>\n</invoke>\n</tool_call>\n\nQuerying...".to_string(),
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

    debug!("XML tool call extraction test complete");
}

#[cfg(feature = "xml")]
#[test_log::test(tokio::test)]
async fn test_multiple_xml_tool_calls() {
    setup();
    debug!("Starting test for multiple XML tool calls");

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

    // Check parameters of the first tool call
    let args1: serde_json::Value = serde_json::from_str(&tool_calls[0].function.arguments)
        .expect("Parameters of the first tool should be valid JSON");
    assert_eq!(
        args1["location"], "Taipei",
        "Location parameter of the first tool should be Taipei"
    );

    // Check parameters of the second tool call
    let args2: serde_json::Value = serde_json::from_str(&tool_calls[1].function.arguments)
        .expect("Parameters of the second tool should be valid JSON");
    assert_eq!(
        args2["expression"], "2+2",
        "Expression parameter of the second tool should be 2+2"
    );

    debug!("Multiple XML tool calls test complete");
}

#[cfg(feature = "xml")]
#[test_log::test(tokio::test)]
async fn test_xml_tool_call_with_complex_parameters() {
    setup();
    debug!("Starting test for XML tool call with complex parameters");

    let message = ChatMessage {
        role: "assistant".to_string(),
        content: "<tool_call>\n<invoke name=\"send_email\">\n<parameter name=\"to\">user@example.com</parameter>\n<parameter name=\"subject\">Test Email</parameter>\n<parameter name=\"body\">This is a test email, including special characters: &lt;test&gt;</parameter>\n<parameter name=\"priority\">high</parameter>\n</invoke>\n</tool_call>".to_string(),
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
        args["body"], "This is a test email, including special characters: <test>",
        "body parameter should be correctly decoded XML entities"
    );
    assert_eq!(args["priority"], "high", "priority parameter should be correct");

    debug!("Complex parameter XML tool call test complete");
}

#[cfg(feature = "xml")]
#[test_log::test(tokio::test)]
async fn test_no_xml_tool_calls() {
    setup();
    debug!("Starting test for case with no XML tool calls");

    let message = ChatMessage {
        role: "assistant".to_string(),
        content: "This is a normal response, with no tool calls.".to_string(),
        attachments: None,
        content_type: "text/plain".to_string(),
    };

    assert!(
        !message.contains_xml_tool_calls(),
        "Should not detect XML tool call"
    );
    let tool_calls = message.extract_xml_tool_calls();
    assert!(tool_calls.is_empty(), "Should not extract any tool calls");

    debug!("No XML tool calls test complete");
}

#[cfg(feature = "xml")]
#[test_log::test(tokio::test)]
async fn test_xml_tool_call_with_empty_parameters() {
    setup();
    debug!("Starting test for XML tool call with no parameters");

    let message = ChatMessage {
        role: "assistant".to_string(),
        content:
            "Execute tool with no parameters.\n\n<tool_call>\n<invoke name=\"get_time\">\n</invoke>\n</tool_call>"
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
    assert_eq!(args.as_object().unwrap().len(), 0, "Parameters object should be empty");

    debug!("XML tool call with no parameters test complete");
}

#[cfg(feature = "xml")]
#[test_log::test(tokio::test)]
async fn test_xml_tool_call_parsing_error_handling() {
    setup();
    debug!("Starting test for XML tool call parsing error handling");

    // Test malformed XML
    let message_with_invalid_xml = ChatMessage {
        role: "assistant".to_string(),
        content: "Malformed XML.\n\n<tool_call>\n<invoke name=\"get_weather\">\n<parameter name=\"location\">Taipei\n</invoke>\n</tool_call>".to_string(),
        attachments: None,
        content_type: "text/plain".to_string(),
    };

    // Even if XML format is incorrect, the function should be able to handle it without crashing
    let tool_calls = message_with_invalid_xml.extract_xml_tool_calls();
    // Due to incorrect XML format, it might not be parsed correctly, but should not crash
    debug!("Malformed XML parsing result: {} tool calls", tool_calls.len());

    debug!("XML tool call parsing error handling test complete");
}

#[cfg(feature = "xml")]
#[test_log::test(tokio::test)]
async fn test_xml_entity_decoding() {
    setup();
    debug!("Starting test for XML entity decoding");

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

    debug!("XML entity decoding test complete");
}

#[cfg(feature = "xml")]
#[test_log::test(tokio::test)]
async fn test_dynamic_xml_tool_call_detection() {
    setup();
    debug!("Starting test for dynamic XML tool call detection");

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

    // Test message containing custom tool tags
    let message_with_custom_tool = ChatMessage {
        role: "assistant".to_string(),
        content: "I need to query weather.\n\n<custom_weather_api>\n<city>Taipei</city>\n</custom_weather_api>\n\nQuerying...".to_string(),
        attachments: None,
        content_type: "text/plain".to_string(),
    };

    // Use tool definition-based detection
    assert!(
        message_with_custom_tool.contains_xml_tool_calls_with_tools(&custom_tools),
        "Should detect custom tool call"
    );

    // Test message containing no tool tags
    let message_without_tools = ChatMessage {
        role: "assistant".to_string(),
        content: "This is a normal response, with no tool calls.".to_string(),
        attachments: None,
        content_type: "text/plain".to_string(),
    };

    assert!(
        !message_without_tools.contains_xml_tool_calls_with_tools(&custom_tools),
        "Should not detect tool call"
    );

    debug!("Dynamic XML tool call detection test complete");
}

#[cfg(feature = "xml")]
#[test_log::test(tokio::test)]
async fn test_dynamic_xml_tool_call_extraction() {
    setup();
    debug!("Starting test for dynamic XML tool call extraction");

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

    // Test message containing custom tool calls
    let message = ChatMessage {
        role: "assistant".to_string(),
        content: "I need to query the database.\n\n<database_query>\n<table>users</table>\n<conditions>age > 18</conditions>\n</database_query>\n\nQuerying...".to_string(),
        attachments: None,
        content_type: "text/plain".to_string(),
    };

    debug!("Testing message content: {}", message.content);
    debug!(
        "Does message contain database_query tag: {}",
        message.content.contains("<database_query>")
    );

    // First test the general method
    let general_tool_calls = message.extract_xml_tool_calls();
    debug!("Number of tool calls extracted by general method: {}", general_tool_calls.len());

    // Then test the method based on tool definitions
    let tool_calls = message.extract_xml_tool_calls_with_tools(&custom_tools);
    debug!("Number of tool calls extracted by tool definition method: {}", tool_calls.len());

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

    debug!("Dynamic XML tool call extraction test complete");
}

#[cfg(feature = "xml")]
#[test_log::test(tokio::test)]
async fn test_potential_tool_name_detection() {
    setup();
    debug!("Starting test for potential tool name detection");

    // Create tool definitions containing fetch_data tool
    let tools_with_fetch_data = vec![ChatTool {
        r#type: "function".to_string(),
        function: FunctionDefinition {
            name: "fetch_data".to_string(),
            description: Some("Get data".to_string()),
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

    // Test message containing potential tool name
    let message_with_potential_tool = ChatMessage {
        role: "assistant".to_string(),
        content: "I need to perform an operation.\n\n<fetch_data>\n<url>https://api.example.com</url>\n</fetch_data>\n\nProcessing...".to_string(),
        attachments: None,
        content_type: "text/plain".to_string(),
    };

    assert!(
        message_with_potential_tool.contains_xml_tool_calls_with_tools(&tools_with_fetch_data),
        "Should detect potential tool call (fetch_data)"
    );

    // Test message containing HTML tags (should not be detected as tool call)
    let message_with_html = ChatMessage {
        role: "assistant".to_string(),
        content: "This is a response containing HTML:\n\n<div>\n<p>This is a paragraph</p>\n</div>".to_string(),
        attachments: None,
        content_type: "text/plain".to_string(),
    };

    assert!(
        !message_with_html.contains_xml_tool_calls_with_tools(&tools_with_fetch_data),
        "Should not detect HTML tags as tool call"
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

    // Test message containing camel-case tool name
    let message_with_camel_case = ChatMessage {
        role: "assistant".to_string(),
        content: "Perform an operation.\n\n<getUserData>\n<userId>123</userId>\n</getUserData>".to_string(),
        attachments: None,
        content_type: "text/plain".to_string(),
    };

    assert!(
        message_with_camel_case.contains_xml_tool_calls_with_tools(&tools_with_get_user_data),
        "Should detect camel-case tool call (getUserData)"
    );

    debug!("Potential tool name detection test complete");
}

#[cfg(feature = "xml")]
#[test_log::test(tokio::test)]
async fn test_mixed_tool_call_formats() {
    setup();
    debug!("Starting test for mixed tool call formats");

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

    // Test message containing multiple formats
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

    // Should be able to parse both tool call formats
    assert!(tool_calls.len() >= 1, "Should extract at least one tool call");

    // Check if standard tool is included
    let has_standard_tool = tool_calls
        .iter()
        .any(|call| call.function.name == "standard_tool");
    assert!(has_standard_tool, "Should include standard_tool call");

    debug!("Mixed tool call formats test complete");
}

#[cfg(feature = "xml")]
#[test_log::test(tokio::test)]
async fn test_remove_xml_tool_calls_with_tool_cells() {
    setup();
    debug!("Starting test for removing XML containing tool calls");

    use crate::client::PoeClient;

    // Test text containing tool calls
    let text_with_tools = r#"I need to query weather information.

<tool_call>
<invoke name="get_weather">
<parameter name="location">Taipei</parameter>
<parameter name="unit">celsius</parameter>
</invoke>
</tool_call>

Please wait a moment, I am querying Taipei's weather for you."#;

    let cleaned_text = PoeClient::remove_xml_tool_calls(text_with_tools);

    // Should remove tool call part
    assert!(
        !cleaned_text.contains("<tool_call>"),
        "Should remove tool_call tag"
    );
    assert!(!cleaned_text.contains("<invoke"), "Should remove invoke tag");
    assert!(
        !cleaned_text.contains("<parameter"),
        "Should remove parameter tag"
    );
    assert!(
        cleaned_text.contains("I need to query weather information."),
        "Should keep normal text"
    );
    assert!(
        cleaned_text.contains("Please wait a moment, I am querying Taipei's weather for you."),
        "Should keep normal text"
    );

    debug!("Remove XML containing tool calls test complete");
}

#[cfg(feature = "xml")]
#[test_log::test(tokio::test)]
async fn test_remove_xml_tool_calls_without_tool_cells() {
    setup();
    debug!("Starting test for removing text without tool calls");

    use crate::client::PoeClient;

    // Test text without tool calls
    let text_without_tools = r#"This is a normal response, with no tool calls.
I can provide general assistance and information for you."#;

    let cleaned_text = PoeClient::remove_xml_tool_calls(text_without_tools);

    // Should keep the original text unchanged
    assert_eq!(
        cleaned_text, text_without_tools,
        "Text without tool calls should remain unchanged"
    );

    debug!("Remove text without tool calls test complete");
}
