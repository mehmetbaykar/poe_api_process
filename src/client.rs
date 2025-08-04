use crate::error::PoeError;
use crate::types::{
    ChatEventType, ChatRequest, ChatResponse, ChatResponseData,
    ChatToolCall, ChatToolResult, FileData, FileUploadRequest, FileUploadResponse,
    FunctionCall, ModelInfo, ModelResponse, PartialToolCall,
};
use futures_util::future::join_all;
use futures_util::{Stream, StreamExt};
use reqwest::header::{HeaderMap, HeaderValue, COOKIE};
use reqwest::Client;
use serde_json::Value;
use std::path::Path;
use std::pin::Pin;
use tokio_util::io::ReaderStream;
#[cfg(feature = "trace")]
use tracing::{debug, warn};

const BASE_URL: &str = "https://api.poe.com/bot/";
const POE_GQL_URL: &str = "https://poe.com/api/gql_POST";
const POE_GQL_MODEL_HASH: &str = "b24b2f2f6da147b3345eec1a433ed17b6e1332df97dea47622868f41078a40cc";
const POE_FILE_UPLOAD_URL: &str = "https://www.quora.com/poe_api/file_upload_3RD_PARTY_POST";

/// Client for interacting with the Poe.com API
///
/// This client provides methods for streaming chat responses, handling tool calls,
/// and uploading files to use with Poe bots.
#[derive(Clone)]
pub struct PoeClient {
    client: Client,
    bot_name: String,
    access_key: String,
}

impl PoeClient {
    pub fn new(bot_name: &str, access_key: &str) -> Self {
        assert!(!bot_name.is_empty(), "bot_name cannot be empty");
        assert!(!access_key.is_empty(), "access_key cannot be empty");
        
        #[cfg(feature = "trace")]
        debug!("Creating new PoeClient instance, bot_name: {}", bot_name);
        Self {
            client: Client::new(),
            bot_name: bot_name.to_string(),
            access_key: access_key.to_string(),
        }
    }

    /// Sends a chat request and returns a stream of responses
    ///
    /// This method sends a chat request to the specified bot and returns a stream
    /// of `ChatResponse` events. The stream will emit events for text responses,
    /// tool calls, errors, and completion.
    ///
    /// # Arguments
    ///
    /// * `request` - The chat request containing messages and optional tool definitions
    ///
    /// # Returns
    ///
    /// A pinned, boxed stream of `Result<ChatResponse, PoeError>` that can be
    /// consumed asynchronously.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use poe_api_process::{PoeClient, ChatRequest, ChatMessage};
    /// use futures_util::StreamExt;
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = PoeClient::new("Llama-4-Scout", "your_access_key");
    /// let request = ChatRequest {
    ///     version: "1.1".to_string(),
    ///     r#type: "query".to_string(),
    ///     query: vec![ChatMessage {
    ///         role: "user".to_string(),
    ///         content: "Hello!".to_string(),
    ///         content_type: "text/markdown".to_string(),
    ///         attachments: None,
    ///     }],
    ///     // ... other fields
    /// #    user_id: String::new(),
    /// #    conversation_id: String::new(),
    /// #    message_id: String::new(),
    /// #    tools: None,
    /// #    tool_calls: None,
    /// #    tool_results: None,
    /// #    temperature: None,
    /// #    logit_bias: None,
    /// #    stop_sequences: None,
    /// };
    ///
    /// let mut stream = client.stream_request(request).await?;
    /// while let Some(response) = stream.next().await {
    ///     // Handle response events
    /// }
    /// # Ok(())
    /// # }
    /// ```
    #[allow(clippy::too_many_lines)]
    pub async fn stream_request(
        &self,
        request: ChatRequest,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<ChatResponse, PoeError>> + Send>>, PoeError> {
        #[cfg(feature = "trace")]
        debug!("Starting stream request, bot_name: {}", self.bot_name);
        let url = format!("{BASE_URL}{}", self.bot_name);
        #[cfg(feature = "trace")]
        debug!("Sending request to URL: {url}");
        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.access_key))
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            #[cfg(feature = "trace")]
            warn!("API request failed, status code: {}", status);
            return Err(PoeError::BotError(format!("API response status code: {status}")));
        }

        #[cfg(feature = "trace")]
        debug!("Successfully received stream response");

        let mut static_buffer = String::new();
        let mut current_event: Option<ChatEventType> = None;
        let mut is_collecting_data = false;
        let mut accumulated_tool_calls: Vec<PartialToolCall> = Vec::new();
        let mut tool_calls_complete = false;

        let stream = response
            .bytes_stream()
            .map(move |result| {
                result.map_err(PoeError::from).map(|chunk| {
                    let chunk_str = String::from_utf8_lossy(&chunk);
                    #[cfg(feature = "trace")]
                    debug!("Processing stream chunk, size: {} bytes", chunk.len());

                    let mut events = Vec::new();
                    static_buffer.push_str(&chunk_str);

                    while let Some(newline_pos) = static_buffer.find('\n') {
                        let line = static_buffer[..newline_pos].trim().to_string();
                        static_buffer = static_buffer[newline_pos + 1..].to_string();

                        if line.is_empty() {
                            current_event = None;
                            is_collecting_data = false;
                            continue;
                        }

                        if line == ": ping" {
                            #[cfg(feature = "trace")]
                            debug!("Received ping signal");
                            continue;
                        }

                        if line.starts_with("event: ") {
                            let event_name = line.trim_start_matches("event: ").trim();
                            #[cfg(feature = "trace")]
                            debug!("Parsing event type: {}", event_name);

                            let event_type = match event_name {
                                "text" => ChatEventType::Text,
                                "replace_response" => ChatEventType::ReplaceResponse,
                                "json" => ChatEventType::Json,
                                "file" => ChatEventType::File,
                                "done" => ChatEventType::Done,
                                "error" => ChatEventType::Error,
                                _ => {
                                    #[cfg(feature = "trace")]
                                    warn!("Received unknown event type: {}", event_name);
                                    continue;
                                }
                            };

                            current_event = Some(event_type);
                            is_collecting_data = false;
                            continue;
                        }

                        if line.starts_with("data: ") {
                            let data = line.trim_start_matches("data: ").trim();
                            #[cfg(feature = "trace")]
                            debug!(
                                "Received event data: {}",
                                if data.len() > 100 { &data[..100] } else { data }
                            );

                            if let Some(ref event_type) = current_event {
                                match event_type {
                                    ChatEventType::Text | ChatEventType::ReplaceResponse => {
                                        if let Ok(json) = serde_json::from_str::<Value>(data) {
                                            if let Some(text) = json.get("text").and_then(Value::as_str) {
                                                #[cfg(feature = "trace")]
                                                debug!("Parsed text data, length: {}", text.len());
                                                events.push(Ok(ChatResponse {
                                                    event: event_type.clone(),
                                                    data: Some(ChatResponseData::Text {
                                                        text: text.to_string(),
                                                    }),
                                                }));
                                            }
                                        } else {
                                            #[cfg(feature = "trace")]
                                            debug!("JSON parsing failed, possibly incomplete data, waiting for more");
                                            is_collecting_data = true;
                                        }
                                    }
                                    ChatEventType::File => {
                                        if let Ok(file_data) = serde_json::from_str::<FileData>(data) {
                                            #[cfg(feature = "trace")]
                                            debug!("Parsed file data: {}", file_data.name);
                                            events.push(Ok(ChatResponse {
                                                event: ChatEventType::File,
                                                data: Some(ChatResponseData::File(file_data)),
                                            }));
                                        } else {
                                            #[cfg(feature = "trace")]
                                            debug!("File data JSON parsing failed, possibly incomplete data, waiting for more");
                                            is_collecting_data = true;
                                        }
                                    }
                                    ChatEventType::Json => {
                                        if let Ok(json) = serde_json::from_str::<Value>(data) {
                                            #[cfg(feature = "trace")]
                                            debug!("Parsed JSON event data");
                                            let finish_reason = json
                                                .get("choices")
                                                .and_then(|choices| choices.get(0))
                                                .and_then(|choice| choice.get("finish_reason"))
                                                .and_then(Value::as_str);

                                            if finish_reason == Some("tool_calls") {
                                                #[cfg(feature = "trace")]
                                                debug!("Detected tool call completion flag");
                                                tool_calls_complete = true;
                                            }

                                            let tool_calls_delta = json
                                                .get("choices")
                                                .and_then(|choices| choices.get(0))
                                                .and_then(|choice| choice.get("delta"))
                                                .and_then(|delta| delta.get("tool_calls"));

                                            if let Some(tool_calls_array) = tool_calls_delta {
                                                #[cfg(feature = "trace")]
                                                debug!("Detected tool call delta");
                                                if let Some(tool_calls) = tool_calls_array.as_array() {
                                                    for tool_call_delta in tool_calls {
                                                        let index = tool_call_delta
                                                            .get("index")
                                                            .and_then(Value::as_u64)
                                                            .unwrap_or(0);
                                                        let index = index as usize;

                                                        while accumulated_tool_calls.len() <= index {
                                                            accumulated_tool_calls.push(PartialToolCall::default());
                                                        }

                                                        if let Some(id) = tool_call_delta
                                                            .get("id")
                                                            .and_then(Value::as_str)
                                                        {
                                                            accumulated_tool_calls[index].id = id.to_string();
                                                        }

                                                        if let Some(type_str) = tool_call_delta
                                                            .get("type")
                                                            .and_then(Value::as_str)
                                                        {
                                                            accumulated_tool_calls[index].r#type = type_str.to_string();
                                                        }

                                                        if let Some(function) = tool_call_delta.get("function") {
                                                            if let Some(name) = function
                                                                .get("name")
                                                                .and_then(Value::as_str)
                                                            {
                                                                accumulated_tool_calls[index].function_name = name.to_string();
                                                            }

                                                            if let Some(args) = function
                                                                .get("arguments")
                                                                .and_then(Value::as_str)
                                                            {
                                                                accumulated_tool_calls[index].function_arguments.push_str(args);
                                                            }
                                                        }
                                                    }
                                                }
                                            } else if !tool_calls_complete {
                                                // If no tool_calls delta and tool calls not yet complete,
                                                // handle as regular JSON
                                                events.push(Ok(ChatResponse {
                                                    event: ChatEventType::Json,
                                                    data: Some(ChatResponseData::Text {
                                                        text: data.to_string(),
                                                    }),
                                                }));
                                            }
                                        } else {
                                            #[cfg(feature = "trace")]
                                            debug!("JSON event parsing failed, possibly incomplete data");
                                            is_collecting_data = true;
                                        }
                                    }
                                    ChatEventType::Done => {
                                        #[cfg(feature = "trace")]
                                        debug!("Received completion event");
                                        events.push(Ok(ChatResponse {
                                            event: ChatEventType::Done,
                                            data: Some(ChatResponseData::Empty),
                                        }));
                                        current_event = None;
                                    }
                                    ChatEventType::Error => {
                                        if let Ok(json) = serde_json::from_str::<Value>(data) {
                                            let text = json
                                                .get("text")
                                                .and_then(Value::as_str)
                                                .unwrap_or("Unknown error");
                                            let allow_retry = json
                                                .get("allow_retry")
                                                .and_then(Value::as_bool)
                                                .unwrap_or(false);

                                            #[cfg(feature = "trace")]
                                            warn!("Received error event: {}, retryable: {}", text, allow_retry);

                                            events.push(Ok(ChatResponse {
                                                event: ChatEventType::Error,
                                                data: Some(ChatResponseData::Error {
                                                    text: text.to_string(),
                                                    allow_retry,
                                                }),
                                            }));
                                        } else {
                                            #[cfg(feature = "trace")]
                                            warn!("Cannot parse error event data: {}", data);
                                        }
                                        current_event = None;
                                    }
                                }
                            } else {
                                #[cfg(feature = "trace")]
                                debug!("Received data but no current event type");
                            }
                        } else if is_collecting_data {
                            // Try to parse accumulated JSON
                            #[cfg(feature = "trace")]
                            debug!("Trying to parse incomplete JSON data: {}", line);

                            if let Some(ref event_type) = current_event {
                                match event_type {
                                    ChatEventType::Text | ChatEventType::ReplaceResponse => {
                                        if let Ok(json) = serde_json::from_str::<Value>(&line) {
                                            if let Some(text) = json.get("text").and_then(Value::as_str) {
                                                #[cfg(feature = "trace")]
                                                debug!("Successfully parsed accumulated JSON text, length: {}", text.len());

                                                events.push(Ok(ChatResponse {
                                                    event: event_type.clone(),
                                                    data: Some(ChatResponseData::Text {
                                                        text: text.to_string(),
                                                    }),
                                                }));
                                                is_collecting_data = false;
                                                current_event = None;
                                            }
                                        }
                                    }
                                    ChatEventType::File => {
                                        if let Ok(file_data) = serde_json::from_str::<FileData>(&line) {
                                            #[cfg(feature = "trace")]
                                            debug!("Successfully parsed accumulated file data: {}", file_data.name);

                                            events.push(Ok(ChatResponse {
                                                event: ChatEventType::File,
                                                data: Some(ChatResponseData::File(file_data)),
                                            }));
                                            is_collecting_data = false;
                                            current_event = None;
                                        }
                                    }
                                    ChatEventType::Json => {
                                        if let Ok(json) = serde_json::from_str::<Value>(&line) {
                                            #[cfg(feature = "trace")]
                                            debug!("Successfully parsed accumulated JSON event data");

                                            // Check for finish_reason: "tool_calls"
                                            let finish_reason = json
                                                .get("choices")
                                                .and_then(|choices| choices.get(0))
                                                .and_then(|choice| choice.get("finish_reason"))
                                                .and_then(Value::as_str);

                                            if finish_reason == Some("tool_calls") {
                                                #[cfg(feature = "trace")]
                                                debug!("Detected tool call completion flag");
                                                tool_calls_complete = true;
                                            }

                                            let tool_calls_delta = json
                                                .get("choices")
                                                .and_then(|choices| choices.get(0))
                                                .and_then(|choice| choice.get("delta"))
                                                .and_then(|delta| delta.get("tool_calls"));

                                            if let Some(tool_calls_array) = tool_calls_delta {
                                                #[cfg(feature = "trace")]
                                                debug!("Detected tool call delta");

                                                if let Some(tool_calls) = tool_calls_array.as_array() {
                                                    for tool_call_delta in tool_calls {
                                                        let index = tool_call_delta
                                                            .get("index")
                                                            .and_then(Value::as_u64)
                                                            .unwrap_or(0);
                                                        let index = index as usize;

                                                        while accumulated_tool_calls.len() <= index {
                                                            accumulated_tool_calls.push(PartialToolCall::default());
                                                        }

                                                        if let Some(id) = tool_call_delta
                                                            .get("id")
                                                            .and_then(Value::as_str)
                                                        {
                                                            accumulated_tool_calls[index].id = id.to_string();
                                                        }

                                                        if let Some(type_str) = tool_call_delta
                                                            .get("type")
                                                            .and_then(Value::as_str)
                                                        {
                                                            accumulated_tool_calls[index].r#type = type_str.to_string();
                                                        }

                                                        if let Some(function) = tool_call_delta.get("function") {
                                                            if let Some(name) = function
                                                                .get("name")
                                                                .and_then(Value::as_str)
                                                            {
                                                                accumulated_tool_calls[index].function_name = name.to_string();
                                                            }

                                                            if let Some(args) = function
                                                                .get("arguments")
                                                                .and_then(Value::as_str)
                                                            {
                                                                accumulated_tool_calls[index].function_arguments.push_str(args);
                                                            }
                                                        }
                                                    }
                                                }

                                                // If tool calls complete, create and send ChatResponse
                                                if tool_calls_complete && !accumulated_tool_calls.is_empty() {
                                                    let complete_tool_calls = accumulated_tool_calls
                                                        .iter()
                                                        .filter(|tc| {
                                                            !tc.id.is_empty() && !tc.function_name.is_empty()
                                                        })
                                                        .map(|tc| ChatToolCall {
                                                            id: tc.id.clone(),
                                                            r#type: tc.r#type.clone(),
                                                            function: FunctionCall {
                                                                name: tc.function_name.clone(),
                                                                arguments: tc.function_arguments.clone(),
                                                            },
                                                        })
                                                        .collect::<Vec<ChatToolCall>>();

                                                    if !complete_tool_calls.is_empty() {
                                                        #[cfg(feature = "trace")]
                                                        debug!("Sending complete tool calls, count: {}", complete_tool_calls.len());

                                                        events.push(Ok(ChatResponse {
                                                            event: ChatEventType::Json,
                                                            data: Some(ChatResponseData::ToolCalls(complete_tool_calls)),
                                                        }));

                                                        // Reset accumulation state
                                                        accumulated_tool_calls.clear();
                                                        tool_calls_complete = false;
                                                    }
                                                }
                                            } else {
                                                // If no tool_calls delta, handle as regular JSON
                                                events.push(Ok(ChatResponse {
                                                    event: ChatEventType::Json,
                                                    data: Some(ChatResponseData::Text {
                                                        text: line.to_string(),
                                                    }),
                                                }));
                                            }

                                            is_collecting_data = false;
                                            current_event = None;
                                        }
                                    }
                                    ChatEventType::Done | ChatEventType::Error => {
                                        // These event types should not have accumulated data
                                        is_collecting_data = false;
                                    }
                                }
                            }
                        }
                    }

                    // After processing all lines in chunk, check if need to send final tool_calls event
                    if tool_calls_complete && !accumulated_tool_calls.is_empty() {
                        let complete_tool_calls = accumulated_tool_calls
                            .iter()
                            .filter(|tc| !tc.id.is_empty() && !tc.function_name.is_empty())
                            .map(|tc| ChatToolCall {
                                id: tc.id.clone(),
                                r#type: tc.r#type.clone(),
                                function: FunctionCall {
                                    name: tc.function_name.clone(),
                                    arguments: tc.function_arguments.clone(),
                                },
                            })
                            .collect::<Vec<ChatToolCall>>();

                        if !complete_tool_calls.is_empty() {
                            #[cfg(feature = "trace")]
                            debug!("Sending final complete tool calls, count: {}", complete_tool_calls.len());

                            events.push(Ok(ChatResponse {
                                event: ChatEventType::Json,
                                data: Some(ChatResponseData::ToolCalls(complete_tool_calls)),
                            }));

                            // Reset state
                            accumulated_tool_calls.clear();
                            tool_calls_complete = false;
                        }
                    }

                    events
                })
            })
            .flat_map(|result| {
                futures_util::stream::iter(match result {
                    Ok(events) => events,
                    Err(e) => {
                        #[cfg(feature = "trace")]
                        warn!("Stream processing error: {}", e);
                        vec![Err(e)]
                    }
                })
            });

        Ok(Box::pin(stream))
    }

    pub async fn send_tool_results(
        &self,
        original_request: ChatRequest,
        tool_calls: Vec<ChatToolCall>,
        tool_results: Vec<ChatToolResult>,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<ChatResponse, PoeError>> + Send>>, PoeError> {
        #[cfg(feature = "trace")]
        debug!("Sending tool call results, bot_name: {}", self.bot_name);

        // Create new request containing tool results
        let mut request = original_request;
        request.tool_calls = Some(tool_calls);
        request.tool_results = Some(tool_results);

        // Send request and handle response
        self.stream_request(request).await
    }

    pub async fn upload_local_file(
        &self,
        file_path: &str,
        mime_type: Option<&str>,
    ) -> Result<FileUploadResponse, PoeError> {
        #[cfg(feature = "trace")]
        debug!(
            "Starting local file upload: {} | MIME type: {:?}",
            file_path, mime_type
        );
        // Check if file exists
        let path = Path::new(file_path);
        if !path.exists() {
            #[cfg(feature = "trace")]
            warn!("File does not exist: {}", file_path);
            return Err(PoeError::FileNotFound(file_path.to_string()));
        }
        
        // Simplified MIME type handling: use provided mime_type or default
        let content_type = mime_type.unwrap_or("application/octet-stream").to_string();
        
        #[cfg(feature = "trace")]
        debug!("Using MIME type: {}", content_type);
        
        // Build multipart form
        let file = tokio::fs::File::open(path).await.map_err(|e| {
            #[cfg(feature = "trace")]
            warn!("Cannot open file: {}", e);
            PoeError::FileReadError(e)
        })?;
        
        let file_part =
            reqwest::multipart::Part::stream(reqwest::Body::wrap_stream(ReaderStream::new(file)))
                .file_name(
                    path.file_name()
                        .and_then(|name| name.to_str())
                        .unwrap_or("file")
                        .to_string(),
                )
                .mime_str(&content_type)
                .map_err(|e| {
                    #[cfg(feature = "trace")]
                    warn!("Failed to set MIME type: {}", e);
                    PoeError::FileUploadFailed(format!("Failed to set MIME type: {e}"))
                })?;
                
        let form = reqwest::multipart::Form::new().part("file", file_part);
            
        // Send request
        self.send_upload_request(form).await
    }

    /// Upload remote file (via URL)
    pub async fn upload_remote_file(
        &self,
        download_url: &str,
    ) -> Result<FileUploadResponse, PoeError> {
        #[cfg(feature = "trace")]
        debug!("Starting remote file upload: {}", download_url);

        // Check URL format
        url::Url::parse(download_url)?;

        // Build multipart form
        let form = reqwest::multipart::Form::new().text("download_url", download_url.to_string());

        // Send request
        self.send_upload_request(form).await
    }

    pub async fn upload_files_batch(
        &self,
        files: Vec<FileUploadRequest>,
    ) -> Result<Vec<FileUploadResponse>, PoeError> {
        #[cfg(feature = "trace")]
        debug!("Starting batch file upload, count: {}", files.len());

        if files.is_empty() {
            return Ok(Vec::new());
        }

        // Create upload task for each file
        let mut upload_tasks = Vec::with_capacity(files.len());

        for file_request in files {
            let task = match file_request {
                FileUploadRequest::LocalFile { file, mime_type } => {
                    let client = self.clone();
                    let file_path = file.clone();
                    tokio::spawn(async move {
                        client
                            .upload_local_file(&file_path, mime_type.as_deref())
                            .await
                    })
                }
                FileUploadRequest::RemoteFile { download_url } => {
                    let client = self.clone();
                    let url = download_url.clone();
                    tokio::spawn(async move { client.upload_remote_file(&url).await })
                }
            };
            upload_tasks.push(task);
        }

        // Wait for all upload tasks to complete
        let results = join_all(upload_tasks).await;

        // Collect results
        let mut upload_responses = Vec::with_capacity(results.len());

        for task_result in results {
            match task_result {
                Ok(upload_result) => match upload_result {
                    Ok(response) => {
                        #[cfg(feature = "trace")]
                        debug!("File upload successful: {}", response.attachment_url);
                        upload_responses.push(response);
                    }
                    Err(e) => {
                        #[cfg(feature = "trace")]
                        warn!("File upload failed: {}", e);
                        return Err(e);
                    }
                },
                Err(e) => {
                    #[cfg(feature = "trace")]
                    warn!("File upload task failed: {}", e);
                    return Err(PoeError::FileUploadFailed(format!("Upload task failed: {e}")));
                }
            }
        }

        #[cfg(feature = "trace")]
        debug!("Batch upload all successful, {} files total", upload_responses.len());

        Ok(upload_responses)
    }

    /// Send file upload request (internal method)
    async fn send_upload_request(
        &self,
        form: reqwest::multipart::Form,
    ) -> Result<FileUploadResponse, PoeError> {
        #[cfg(feature = "trace")]
        debug!("Sending file upload request to {}", POE_FILE_UPLOAD_URL);

        let response = self
            .client
            .post(POE_FILE_UPLOAD_URL)
            .header("Authorization", format!("Bearer {}", self.access_key))
            .multipart(form)
            .send()
            .await
            .map_err(|e| {
                #[cfg(feature = "trace")]
                warn!("File upload request failed: {}", e);
                PoeError::RequestFailed(e)
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response
                .text()
                .await
                .unwrap_or_else(|_| "Cannot read response content".to_string());

            #[cfg(feature = "trace")]
            warn!("File upload API response error - status: {}, content: {}", status, text);

            return Err(PoeError::FileUploadFailed(format!(
                "Upload failed - status: {status}, content: {text}"
            )));
        }

        #[cfg(feature = "trace")]
        debug!("Successfully received file upload response");

        let response_text = response.text().await.map_err(|e| {
            #[cfg(feature = "trace")]
            warn!("Failed to read file upload response content: {}", e);
            PoeError::RequestFailed(e)
        })?;

        #[cfg(feature = "trace")]
        debug!("File upload response content: {}", response_text);

        let upload_response: FileUploadResponse =
            serde_json::from_str(&response_text).map_err(|e| {
                #[cfg(feature = "trace")]
                warn!("Failed to parse file upload response: {}", e);
                PoeError::JsonParseFailed(e)
            })?;

        #[cfg(feature = "trace")]
        debug!("File upload successful, attachment URL: {}", upload_response.attachment_url);

        Ok(upload_response)
    }
}

pub async fn get_model_list(language_code: Option<&str>) -> Result<ModelResponse, PoeError> {
    #[cfg(feature = "trace")]
    debug!("Starting to get model list, language code: {:?}", language_code);

    let client = Client::builder()
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
        .build()
        .map_err(|e| {
            #[cfg(feature = "trace")]
            warn!("Failed to create HTTP client: {}", e);
            PoeError::BotError(e.to_string())
        })?;

    let payload = serde_json::json!({
        "queryName": "ExploreBotsListPaginationQuery",
        "variables": {
            "categoryName": "defaultCategory",
            "count": 150
        },
        "extensions": {
            "hash": POE_GQL_MODEL_HASH
        }
    });

    #[cfg(feature = "trace")]
    debug!("Preparing GraphQL request payload, using hash: {}", POE_GQL_MODEL_HASH);

    let mut headers = HeaderMap::new();
    headers.insert("Content-Type", HeaderValue::from_static("application/json"));
    headers.insert("Accept", HeaderValue::from_static("*/*"));
    headers.insert(
        "Accept-Language",
        HeaderValue::from_static("en-US,en;q=0.9"),
    );
    headers.insert("Origin", HeaderValue::from_static("https://poe.com"));
    headers.insert("Referer", HeaderValue::from_static("https://poe.com"));
    headers.insert("Sec-Fetch-Dest", HeaderValue::from_static("empty"));
    headers.insert("Sec-Fetch-Mode", HeaderValue::from_static("cors"));
    headers.insert("Sec-Fetch-Site", HeaderValue::from_static("same-origin"));
    headers.insert("poegraphql", HeaderValue::from_static("1"));

    if let Some(code) = language_code {
        let cookie_value = format!("Poe-Language-Code={code}; p-b=1");
        #[cfg(feature = "trace")]
        debug!("Setting language cookie: {}", cookie_value);

        headers.insert(
            COOKIE,
            HeaderValue::from_str(&cookie_value).map_err(|e| {
                #[cfg(feature = "trace")]
                warn!("Failed to set cookie: {}", e);
                PoeError::BotError(e.to_string())
            })?,
        );
    }

    #[cfg(feature = "trace")]
    debug!("Sending GraphQL request to {}", POE_GQL_URL);

    let response = client
        .post(POE_GQL_URL)
        .headers(headers)
        .json(&payload)
        .send()
        .await
        .map_err(|e| {
            #[cfg(feature = "trace")]
            warn!("Failed to send GraphQL request: {}", e);
            PoeError::RequestFailed(e)
        })?;

    if !response.status().is_success() {
        let status = response.status();
        let text = response
            .text()
            .await
            .unwrap_or_else(|_| "Cannot read response content".to_string());

        #[cfg(feature = "trace")]
        warn!("GraphQL API response error - status: {}, content: {}", status, text);

        return Err(PoeError::BotError(format!(
            "API response error - status: {status}, content: {text}"
        )));
    }

    #[cfg(feature = "trace")]
    debug!("Successfully received GraphQL response");

    let json_value = response.text().await.map_err(|e| {
        #[cfg(feature = "trace")]
        warn!("Failed to read GraphQL response content: {}", e);
        PoeError::RequestFailed(e)
    })?;

    let data: Value = serde_json::from_str(&json_value).map_err(|e| {
        #[cfg(feature = "trace")]
        warn!("Failed to parse GraphQL response JSON: {}", e);
        PoeError::JsonParseFailed(e)
    })?;

    let mut model_list = Vec::with_capacity(150);

    if let Some(edges) = data["data"]["exploreBotsConnection"]["edges"].as_array() {
        #[cfg(feature = "trace")]
        debug!("Found {} model nodes", edges.len());

        for edge in edges {
            if let Some(handle) = edge["node"]["handle"].as_str() {
                #[cfg(feature = "trace")]
                debug!("Parsing model ID: {}", handle);

                model_list.push(ModelInfo {
                    id: handle.to_string(),
                    object: "model".to_string(),
                    created: 0,
                    owned_by: "poe".to_string(),
                });
            } else {
                #[cfg(feature = "trace")]
                debug!("Cannot find handle field in model node");
            }
        }
    } else {
        #[cfg(feature = "trace")]
        warn!("Cannot get model list nodes from response");
        return Err(PoeError::BotError("Cannot get model list from response".to_string()));
    }

    if model_list.is_empty() {
        #[cfg(feature = "trace")]
        warn!("Retrieved model list is empty");
        return Err(PoeError::BotError("Retrieved model list is empty".to_string()));
    }

    #[cfg(feature = "trace")]
    debug!("Successfully parsed {} models", model_list.len());

    Ok(ModelResponse { data: model_list })
}

