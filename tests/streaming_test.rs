mod common;

use poe_api_process::{ChatEventType, ChatMessage, ChatRequest, ChatResponseData, PoeClient};
use futures_util::StreamExt;

#[tokio::test]
async fn test_stream_request() {
    common::setup();
    let access_key = common::get_access_key();
    let client = PoeClient::new("Llama-4-Scout", &access_key);

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

    let result = client.stream_request(request).await;

    assert!(result.is_ok(), "Creating stream request should succeed");

    if let Ok(mut stream) = result {
        let mut received_response = false;

        while let Some(response) = stream.next().await {
            match response {
                Ok(_event) => {
                    received_response = true;
                }
                Err(e) => {
                    panic!("Stream processing error: {e}");
                }
            }
        }

        assert!(received_response, "Should receive at least one response");
    }
}

#[tokio::test]
async fn test_stream_content_verification() {
    common::setup();
    let access_key = common::get_access_key();
    let client = PoeClient::new("Llama-4-Scout", &access_key);

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

    let result = client.stream_request(request).await;

    assert!(result.is_ok(), "Creating stream request should succeed");

    if let Ok(mut stream) = result {
        let mut received_text_event = false;

        while let Some(response) = stream.next().await {
            match response {
                Ok(event_response) => {
                    match event_response.event {
                        ChatEventType::Text => {
                            received_text_event = true;
                            if let Some(ChatResponseData::Text { text: _ }) = event_response.data {
                                // Text received
                            }
                        }
                        ChatEventType::Error => {
                            if let Some(ChatResponseData::Error { text, allow_retry: _ }) =
                                event_response.data
                            {
                                panic!("Stream processing received Error event: {text}");
                            }
                        }
                        _ => {}
                    }
                }
                Err(e) => {
                    panic!("Stream processing error: {e}");
                }
            }
        }

        assert!(received_text_event, "Should receive at least one Event::Text event");
    }
}