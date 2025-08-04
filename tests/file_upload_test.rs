mod common;

use poe_api_process::{Attachment, ChatMessage, ChatRequest, ChatResponseData, FileUploadRequest, PoeClient};
use futures_util::StreamExt;
use std::fs::File;
use std::io::Write;
use tempfile::tempdir;

#[tokio::test]
async fn test_file_upload() {
    common::setup();
    let access_key = common::get_access_key();
    let client = PoeClient::new("Llama-4-Scout", &access_key);
    
    // Create a temporary file for testing
    let temp_dir = tempdir().expect("Cannot create temporary directory");
    let file_path = temp_dir.path().join("test_upload.txt");
    let file_path_str = file_path.to_str().unwrap().to_string();
    {
        let mut file = File::create(&file_path).expect("Cannot create temporary file");
        writeln!(file, "This is test upload file content").expect("Cannot write to temporary file");
    }
    
    // Test local file upload
    let upload_result = client.upload_local_file(&file_path_str, None).await;
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
    let batch_upload_requests = vec![
        FileUploadRequest::LocalFile {
            file: file_path_str.clone(),
            mime_type: None,
        },
    ];
    let batch_result = client.upload_files_batch(batch_upload_requests).await;
    assert!(batch_result.is_ok(), "Batch upload should succeed");
    if let Ok(responses) = batch_result {
        assert!(!responses.is_empty(), "Should upload at least one file");
        assert!(
            !responses[0].attachment_url.is_empty(),
            "Batch upload attachment URL should not be empty"
        );
    }
    
    // Test sending message with attachment
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
    let result = client.stream_request(request).await;
    assert!(result.is_ok(), "Message request with attachment should succeed");
    if let Ok(mut stream) = result {
        let mut received_response = false;
        while let Some(response) = stream.next().await {
            match response {
                Ok(event) => {
                    received_response = true;
                    // Check if response mentions attachment or file
                    if let Some(ChatResponseData::Text { text }) = &event.data {
                        if text.contains("file") || text.contains("content") {
                            // Response mentions file
                        }
                    }
                }
                Err(e) => {
                    panic!("Stream processing error with attachments: {e}");
                }
            }
        }
        assert!(received_response, "Should receive at least one response for message with attachment");
    }
    
    // Test invalid file path
    let invalid_path = "non_existent_file_path.txt";
    let invalid_result = client.upload_local_file(invalid_path, None).await;
    assert!(invalid_result.is_err(), "Upload of non-existent file should fail");
    
    // Clean up temporary file
    temp_dir.close().expect("Cannot clean up temporary directory");
}

#[tokio::test]
async fn test_remote_file_upload() {
    common::setup();
    let access_key = common::get_access_key();
    let client = PoeClient::new("Llama-4-Scout", &access_key);

    // Use publicly accessible test file URL
    let test_url = "https://www.w3.org/WAI/ER/tests/xhtml/testfiles/resources/pdf/dummy.pdf";

    let upload_result = client.upload_remote_file(test_url).await;

    // Note: Since remote server may be unreliable, we don't assert this must succeed
    // But if successful, we check if response format is correct
    if let Ok(response) = upload_result {
        assert!(!response.attachment_url.is_empty(), "Attachment URL should not be empty");
        assert!(!response.mime_type.unwrap().is_empty(), "MIME type should not be empty");
        assert!(response.size.unwrap() > 0, "File size should be greater than 0");
    }

    // Test invalid URL
    let invalid_url = "invalid-url";
    let invalid_result = client.upload_remote_file(invalid_url).await;
    assert!(invalid_result.is_err(), "Uploading invalid URL should fail");
}