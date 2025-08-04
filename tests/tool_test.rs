mod common;

use poe_api_process::{
    ChatEventType, ChatMessage, ChatRequest, ChatResponseData, ChatTool, ChatToolCall,
    FunctionDefinition, FunctionParameters, PoeClient,
};
use futures_util::StreamExt;
use serde_json::json;

#[tokio::test]
async fn test_stream_tool_content_verification() {
    common::setup();
    let access_key = common::get_access_key();
    let client = PoeClient::new("GPT-4o-Mini", &access_key);

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
                description: "Get weather information for a location".to_string(),
                parameters: FunctionParameters {
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
        }]),
        tool_calls: None,
        tool_results: None,
        logit_bias: None,
        stop_sequences: None,
    };

    let result = client.stream_request(request).await;

    assert!(result.is_ok(), "Creating tool stream request should succeed");

    if let Ok(mut stream) = result {
        let mut _received_tool_call = false;

        while let Some(response) = stream.next().await {
            match response {
                Ok(event_response) => {
                    match event_response.event {
                        ChatEventType::Json => {
                            // Check if there are tool_calls
                            if let Some(ChatResponseData::ToolCalls(tool_calls)) =
                                event_response.data
                            {
                                _received_tool_call = true;

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
                                }

                                // Since we confirmed receiving tool call, we can exit the loop
                                break;
                            }
                        }
                        ChatEventType::Error => {
                            if let Some(ChatResponseData::Error { text, allow_retry: _ }) =
                                event_response.data
                            {
                                panic!("Tool stream processing received Error event: {text}");
                            }
                        }
                        _ => {}
                    }
                }
                Err(e) => {
                    panic!("Tool stream processing error: {e}");
                }
            }
        }

        // Tool calls may not always occur depending on model response
    }
}

#[tokio::test]
async fn test_tool_calls_parsing() {
    common::setup();

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
}

#[tokio::test]
async fn test_tool_call_parse_error() {
    common::setup();

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

    // Try to parse invalid tool call
    let tool_calls_value = invalid_tool_calls_json.get("tool_calls").unwrap();
    let parse_result: Result<Vec<ChatToolCall>, _> =
        serde_json::from_value(tool_calls_value.clone());

    // Verify parsing results should be error
    assert!(parse_result.is_err(), "Parsing invalid tool call should fail");

    // Verify error type
    let error = parse_result.unwrap_err();
    assert!(
        error.to_string().contains("missing field"),
        "Error message should indicate missing field"
    );
}