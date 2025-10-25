use crate::error::PoeError;
use crate::types::*;
#[cfg(feature = "trace")]
use crate::logging::{redact_header, truncate_str_by_bytes, loggable_request_json};
use futures_util::Stream;
use futures_util::StreamExt;
use futures_util::future::join_all;
use reqwest::Client;
use reqwest::header::{COOKIE, HeaderMap, HeaderValue};
use serde_json::Value;
use std::path::Path;
use std::pin::Pin;
use tokio_util::io::ReaderStream;
#[cfg(feature = "trace")]
use tracing::{debug, warn};

const POE_GQL_URL: &str = "https://poe.com/api/gql_POST";
const POE_GQL_MODEL_HASH: &str = "b24b2f2f6da147b3345eec1a433ed17b6e1332df97dea47622868f41078a40cc";
const POE_GQL_MODEL_REVISION: &str = "e2acc7025b43e08e88164ba8105273f37fbeaa26";

#[derive(Clone)]
pub struct PoeClient {
    client: Client,
    bot_name: String,
    access_key: String,
    poe_base_url: String,
    poe_file_upload_url: String,
}

impl PoeClient {
    pub fn new(
        bot_name: &str,
        access_key: &str,
        poe_base_url: &str,
        poe_file_upload_url: &str,
    ) -> Self {
        #[cfg(feature = "trace")]
        debug!("Creating new PoeClient instance, bot_name: {}", bot_name);

        // Handle trailing slashes in URLs
        let normalized_base_url = if poe_base_url.ends_with('/') {
            poe_base_url.trim_end_matches('/').to_string()
        } else {
            poe_base_url.to_string()
        };

        let normalized_file_upload_url = if poe_file_upload_url.ends_with('/') {
            poe_file_upload_url.trim_end_matches('/').to_string()
        } else {
            poe_file_upload_url.to_string()
        };

        Self {
            client: Client::new(),
            bot_name: bot_name.to_string(),
            access_key: access_key.to_string(),
            poe_base_url: normalized_base_url,
            poe_file_upload_url: normalized_file_upload_url,
        }
    }

    pub async fn stream_request(
        &self,
        #[cfg(feature = "xml")] mut request: ChatRequest,
        #[cfg(not(feature = "xml"))] request: ChatRequest,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<ChatResponse, PoeError>> + Send>>, PoeError> {
        #[cfg(feature = "trace")]
        debug!("Starting streaming request, bot_name: {}", self.bot_name);

        // When xml feature is enabled, automatically convert tools to XML format
        #[cfg(feature = "xml")]
        {
            if request.tools.is_some() {
                #[cfg(feature = "trace")]
                debug!("XML feature detected, automatically converting tools to XML format");

                // Use methods from xml module
                request.append_tools_as_xml();
                request.tools = None; // Clear original tool definitions
            }

            // If there are tool results, also convert to XML format and clear original data
            if request.tool_results.is_some() {
                #[cfg(feature = "trace")]
                debug!("XML feature detected, automatically converting tool results to XML format");

                // Convert tool results to XML format and append to end of message
                request.append_tool_results_as_xml();

                // Clear original tool calls and results since they are converted to XML format
                request.tool_calls = None;
                request.tool_results = None;
            }
        }

        let url = format!("{}/bot/{}", self.poe_base_url, self.bot_name);
        
        #[cfg(feature = "trace")]
        debug!("Sending request to URL: {}", url);

        // Log outbound request with structured data
        #[cfg(feature = "trace")]
        {
            let request_json = serde_json::to_value(&request).unwrap_or(Value::Null);
            let loggable_request = loggable_request_json(&request_json, 64 * 1024); // 64KB max
            
            // Create redacted headers map
            let mut headers_map = std::collections::HashMap::new();
            headers_map.insert("Authorization", redact_header("Authorization", "<Bearer token>"));
            headers_map.insert("Content-Type", "application/json".to_string());
            
            debug!("outbound_request method={}, url={}, headers_redacted={:?}, body_pretty={}", 
                "POST",
                url.as_str(),
                headers_map,
                serde_json::to_string_pretty(&loggable_request).unwrap_or_else(|_| "Failed to serialize".to_string())
            );
        }

        #[cfg(feature = "trace")]
        debug!(
            "🔍 Full request body being sent: {}",
            serde_json::to_string_pretty(&request).unwrap_or_else(|_| "Failed to serialize".to_string())
        );

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
            return Err(PoeError::BotError(format!("API response status code: {}", status)));
        }

        #[cfg(feature = "trace")]
        debug!("Successfully received streaming response");

        let mut static_buffer = String::new();
        let mut current_event: Option<ChatEventType> = None;
        let mut is_collecting_data = false;
        // State for accumulating tool_calls
        let mut accumulated_tool_calls: Vec<PartialToolCall> = Vec::new();
        let mut tool_calls_complete = false;

        // XML tool call buffering and detection state
        #[cfg(feature = "xml")]
        let mut xml_text_buffer = String::new();
        #[cfg(feature = "xml")]
        let mut xml_detection_active = false;
        #[cfg(feature = "xml")]
        let available_tools = request.tools.clone().unwrap_or_default();

        let stream = response
            .bytes_stream()
            .map(move |result| {
                result.map_err(PoeError::from).map(|chunk| {
                    let chunk_str = String::from_utf8_lossy(&chunk);
                    #[cfg(feature = "trace")]
                    debug!("Processing stream chunk, size: {} bytes", chunk.len());

                    let mut events = Vec::new();
                    // Add new chunk to static buffer
                    static_buffer.push_str(&chunk_str);

                    // Find complete messages
                    while let Some(newline_pos) = static_buffer.find('\n') {
                        let line = static_buffer[..newline_pos].trim().to_string();
                        static_buffer = static_buffer[newline_pos + 1..].to_string();

                        if line.is_empty() {
                            // Reset current event state, prepare to process next event
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

                                                // Log text event with potential truncation
                                                #[cfg(feature = "trace")]
                                                {
                                                    let (truncated_text, was_truncated) = truncate_str_by_bytes(text, 64 * 1024);
                                                    let loggable_text = if was_truncated { truncated_text } else { text.to_string() };
                                                    debug!("incoming_text_event event_type={:?}, text_preview={}, original_length={}", 
                                                        event_type,
                                                        loggable_text.as_str(),
                                                        text.len()
                                                    );
                                                }

                                                // XML tool call detection and buffering logic
                                                #[cfg(feature = "xml")]
                                                {
                                                    // Smart detection based on actual tool definitions
                                                    let should_start_xml_detection = !xml_detection_active && (
                                                        text.contains("<function_call>") ||
                                                        text.contains("<invoke") ||
                                                        // Check if any defined tool name tags are present
                                                        available_tools.iter().any(|tool|
                                                            text.contains(&format!("<{}>", tool.function.name))
                                                        )
                                                    );
                                                    if should_start_xml_detection {
                                                        xml_detection_active = true;
                                                        xml_text_buffer.clear();
                                                        #[cfg(feature = "trace")]
                                                        debug!("Detected XML tool call with defined tools, starting XML buffering | Clearing buffer to restart");
                                                    }
                                                    if xml_detection_active {
                                                        xml_text_buffer.push_str(text);
                                                        #[cfg(feature = "trace")]
                                                        debug!("XML mode: Text added to buffer | Length: {}", xml_text_buffer.len());
                                                        // Check for complete tool calls
                                                        let message = ChatMessage {
                                                            role: "assistant".to_string(),
                                                            content: xml_text_buffer.clone(),
                                                            attachments: None,
                                                            content_type: "text/plain".to_string(),
                                                        };
                                                        // Use tool definitions to detect and parse
                                                        if message.contains_xml_tool_calls_with_tools(&available_tools) {
                                                            let tool_calls = message.extract_xml_tool_calls_with_tools(&available_tools);
                                                            if !tool_calls.is_empty() {
                                                                #[cfg(feature = "trace")]
                                                                debug!("Detected complete XML tool calls, converting to standard format, count: {}", tool_calls.len());
                                                                // Send tool call event
                                                                events.push(Ok(ChatResponse {
                                                                    event: ChatEventType::Json,
                                                                    data: Some(ChatResponseData::ToolCalls(tool_calls)),
                                                                }));
                                                                // Remove XML part and send remaining text
                                                                let clean_text = Self::remove_xml_tool_calls(&xml_text_buffer);
                                                                if !clean_text.trim().is_empty() {
                                                                    events.push(Ok(ChatResponse {
                                                                        event: event_type.clone(),
                                                                        data: Some(ChatResponseData::Text {
                                                                            text: clean_text,
                                                                        }),
                                                                    }));
                                                                }
                                                                // Reset XML buffer state
                                                                xml_text_buffer.clear();
                                                                xml_detection_active = false;
                                                            } else {
                                                                // No complete tool calls, continue buffering
                                                                #[cfg(feature = "trace")]
                                                                debug!("XML tool calls not yet complete, continuing buffering");
                                                            }
                                                        } else {
                                                            // Check if buffer should be released
                                                            let should_release = xml_text_buffer.contains('\n') &&
                                                                 xml_text_buffer.len() > 200 &&
                                                                 !available_tools.iter().any(|tool|
                                                                     xml_text_buffer.contains(&format!("<{}>", tool.function.name)) ||
                                                                     xml_text_buffer.contains(&format!("</{}>", tool.function.name))
                                                                 ) &&
                                                                 !xml_text_buffer.contains("<function_call>") &&
                                                                 !xml_text_buffer.contains("<invoke");
                                                            if should_release {
                                                                #[cfg(feature = "trace")]
                                                                debug!("XML buffer too large or no tool calls, sending as plain text");
                                                                // Send buffered text
                                                                events.push(Ok(ChatResponse {
                                                                    event: event_type.clone(),
                                                                    data: Some(ChatResponseData::Text {
                                                                        text: xml_text_buffer.clone(),
                                                                    }),
                                                                }));
                                                                // Reset buffer state
                                                                xml_text_buffer.clear();
                                                                xml_detection_active = false;
                                                            } else {
                                                                // Continue buffering
                                                                #[cfg(feature = "trace")]
                                                                debug!("Continuing to buffer XML text, current length: {}", xml_text_buffer.len());
                                                            }
                                                        }
                                                    } else {
                                                        // No XML detected, send text directly
                                                        events.push(Ok(ChatResponse {
                                                            event: event_type.clone(),
                                                            data: Some(ChatResponseData::Text {
                                                                text: text.to_string(),
                                                            }),
                                                        }));
                                                    }
                                                }

                                                #[cfg(not(feature = "xml"))]
                                                {
                                                    events.push(Ok(ChatResponse {
                                                        event: event_type.clone(),
                                                        data: Some(ChatResponseData::Text {
                                                            text: text.to_string(),
                                                        }),
                                                    }));
                                                }
                                            }
                                        } else {
                                            #[cfg(feature = "trace")]
                                            debug!("JSON parsing failed, might be incomplete data, waiting for more data");
                                            is_collecting_data = true;
                                        }
                                    }
                                    ChatEventType::File => {
                                        if let Ok(file_data) = serde_json::from_str::<FileData>(data) {
                                            #[cfg(feature = "trace")]
                                            debug!("Parsed file data: {}", file_data.name);
                                            
                                            // Log file event
                                            #[cfg(feature = "trace")]
                                            debug!("incoming_file_event file_name={}, content_type={}, url_length={}", 
                                                file_data.name.as_str(),
                                                file_data.content_type.as_str(),
                                                file_data.url.len()
                                            );
                                            
                                            events.push(Ok(ChatResponse {
                                                event: ChatEventType::File,
                                                data: Some(ChatResponseData::File(file_data)),
                                            }));
                                        } else {
                                            #[cfg(feature = "trace")]
                                            debug!("File data JSON parsing failed, might be incomplete data, waiting for more data");
                                            is_collecting_data = true;
                                        }
                                    }
                                    ChatEventType::Json => {
                                        if let Ok(json) = serde_json::from_str::<Value>(data) {
                                            #[cfg(feature = "trace")]
                                            debug!("Parsed JSON event data");
                                            
                                            // Log JSON event data with truncation
                                            #[cfg(feature = "trace")]
                                            {
                                                let loggable_json = crate::logging::truncate_text_fields(&json, 64 * 1024);
                                                debug!("incoming_json_event json_pretty={}", 
                                                    serde_json::to_string_pretty(&loggable_json).unwrap_or_else(|_| "Failed to serialize".to_string())
                                                );
                                            }
                                            
                                            // Check for finish_reason: "tool_calls", indicating tool calls are complete
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

                                            // Check for tool_calls delta
                                            let tool_calls_delta = json
                                                .get("choices")
                                                .and_then(|choices| choices.get(0))
                                                .and_then(|choice| choice.get("delta"))
                                                .and_then(|delta| delta.get("tool_calls"));

                                            if let Some(tool_calls_array) = tool_calls_delta {
                                                #[cfg(feature = "trace")]
                                                debug!("Detected tool call delta");
                                                // Process each tool call delta
                                                if let Some(tool_calls) = tool_calls_array.as_array() {
                                                    for tool_call_delta in tool_calls {
                                                        let index = tool_call_delta
                                                            .get("index")
                                                            .and_then(Value::as_u64)
                                                            .unwrap_or(0)
                                                            as usize;

                                                        // Ensure accumulated_tool_calls has enough elements
                                                        while accumulated_tool_calls.len() <= index {
                                                            accumulated_tool_calls.push(PartialToolCall::default());
                                                        }

                                                        // Update id and type
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

                                                        // Update function-related fields
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
                                                // If no tool_calls delta and tool calls are not complete,
                                                // process as general JSON
                                                events.push(Ok(ChatResponse {
                                                    event: ChatEventType::Json,
                                                    data: Some(ChatResponseData::Text {
                                                        text: data.to_string(),
                                                    }),
                                                }));
                                            }
                                        } else {
                                            #[cfg(feature = "trace")]
                                            debug!("JSON event parsing failed, might be incomplete data");
                                            is_collecting_data = true;
                                        }
                                    }
                                    ChatEventType::Done => {
                                        #[cfg(feature = "trace")]
                                        debug!("Received done event");
                                        
                                        // Log done event
                                        #[cfg(feature = "trace")]
                                        debug!("incoming_done_event event_type=done");
                                        
                                        // Process any remaining XML buffer content
                                        #[cfg(feature = "xml")]
                                        {
                                            if xml_detection_active && !xml_text_buffer.trim().is_empty() {
                                                #[cfg(feature = "trace")]
                                                debug!("Processing remaining XML buffer content, length: {}", xml_text_buffer.len());
                                                let message = ChatMessage {
                                                    role: "assistant".to_string(),
                                                    content: xml_text_buffer.clone(),
                                                    attachments: None,
                                                    content_type: "text/plain".to_string(),
                                                };
                                                // Use tool definitions to detect and parse
                                                if message.contains_xml_tool_calls_with_tools(&available_tools) {
                                                    let tool_calls = message.extract_xml_tool_calls_with_tools(&available_tools);
                                                    if !tool_calls.is_empty() {
                                                        #[cfg(feature = "trace")]
                                                        debug!("Detected XML tool calls in done event, count: {}", tool_calls.len());
                                                        // Send tool call event
                                                        events.push(Ok(ChatResponse {
                                                            event: ChatEventType::Json,
                                                            data: Some(ChatResponseData::ToolCalls(tool_calls)),
                                                        }));
                                                        // Send cleaned text (if any)
                                                        let clean_text = Self::remove_xml_tool_calls(&xml_text_buffer);
                                                        if !clean_text.trim().is_empty() {
                                                            events.push(Ok(ChatResponse {
                                                                event: ChatEventType::Text,
                                                                data: Some(ChatResponseData::Text {
                                                                    text: clean_text,
                                                                }),
                                                            }));
                                                        }
                                                    } else {
                                                        // Send as plain text
                                                        events.push(Ok(ChatResponse {
                                                            event: ChatEventType::Text,
                                                            data: Some(ChatResponseData::Text {
                                                                text: xml_text_buffer.clone(),
                                                            }),
                                                        }));
                                                    }
                                                } else {
                                                    // Send as plain text
                                                    events.push(Ok(ChatResponse {
                                                        event: ChatEventType::Text,
                                                        data: Some(ChatResponseData::Text {
                                                            text: xml_text_buffer.clone(),
                                                        }),
                                                    }));
                                                }
                                                // Clear buffer state
                                                xml_text_buffer.clear();
                                                xml_detection_active = false;
                                            }
                                        }
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
                                            warn!("Received error event: {}, Retryable: {}", text, allow_retry);
                                            
                                            // Log error event
                                            #[cfg(feature = "trace")]
                                            debug!("incoming_error_event error_text={}, retryable={}", 
                                                text,
                                                allow_retry
                                            );

                                            events.push(Ok(ChatResponse {
                                                event: ChatEventType::Error,
                                                data: Some(ChatResponseData::Error {
                                                    text: text.to_string(),
                                                    allow_retry,
                                                }),
                                            }));
                                        } else {
                                            #[cfg(feature = "trace")]
                                            warn!("Could not parse error event data: {}", data);
                                        }
                                        current_event = None;
                                    }
                                }
                            } else {
                                #[cfg(feature = "trace")]
                                debug!("Received data but no current event type");
                            }
                        } else if is_collecting_data {
                            // Attempt to parse accumulated JSON
                            #[cfg(feature = "trace")]
                            debug!("Attempting to parse incomplete JSON data: {}", line);

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

                                            // Check for tool_calls delta
                                            let tool_calls_delta = json
                                                .get("choices")
                                                .and_then(|choices| choices.get(0))
                                                .and_then(|choice| choice.get("delta"))
                                                .and_then(|delta| delta.get("tool_calls"));

                                            if let Some(tool_calls_array) = tool_calls_delta {
                                                #[cfg(feature = "trace")]
                                                debug!("Detected tool call delta");

                                                // Process each tool call delta
                                                if let Some(tool_calls) = tool_calls_array.as_array() {
                                                    for tool_call_delta in tool_calls {
                                                        let index = tool_call_delta
                                                            .get("index")
                                                            .and_then(Value::as_u64)
                                                            .unwrap_or(0)
                                                            as usize;

                                                        // Ensure accumulated_tool_calls has enough elements
                                                        while accumulated_tool_calls.len() <= index {
                                                            accumulated_tool_calls.push(PartialToolCall::default());
                                                        }

                                                        // Update id and type
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

                                                        // Update function-related fields
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

                                                // If tool calls are complete, create and send ChatResponse
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

                                                        // Reset accumulated state
                                                        accumulated_tool_calls.clear();
                                                        tool_calls_complete = false;
                                                    }
                                                }
                                            } else {
                                                // If no tool_calls delta, process as general JSON
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

                    // After processing all lines in the chunk, check if a final tool_calls event needs to be sent
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
                    Ok(events) => {
                        // Log each yielded ChatResponse
                        #[cfg(feature = "trace")]
                        for event in &events {
                            if let Ok(response) = event {
                                debug!("yielding_response event_type={:?}, has_data={}", 
                                    response.event,
                                    response.data.is_some()
                                );
                            }
                        }
                        events
                    }
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

        // Create a new request containing tool results
        let mut request = original_request;

        // When xml feature is enabled, append tool results in XML format to the end of the message
        #[cfg(feature = "xml")]
        {
            #[cfg(feature = "trace")]
            debug!("XML feature detected, converting tool results to XML format and appending to the end of the message");

            // Set tool calls and results first so XML conversion methods can access them
            request.tool_calls = Some(tool_calls);
            request.tool_results = Some(tool_results);

            // Convert tool results to XML format and append to the end of the message
            request.append_tool_results_as_xml();

            // Clear original tool calls and results since they are converted to XML format
            request.tool_calls = None;
            request.tool_results = None;

            #[cfg(feature = "trace")]
            debug!(
                "🔧 Tool results XML conversion complete, checking message content: {}",
                request
                    .query
                    .iter()
                    .map(|msg| format!("Role: {}, Content length: {}", msg.role, msg.content.len()))
                    .collect::<Vec<_>>()
                    .join("; ")
            );
        }

        // When xml feature is not enabled, use the original JSON API approach
        #[cfg(not(feature = "xml"))]
        {
            request.tool_calls = Some(tool_calls);
            request.tool_results = Some(tool_results);
        }

        #[cfg(feature = "trace")]
        debug!(
            "Tool results request structure: {}",
            serde_json::to_string_pretty(&request).unwrap_or_else(|_| "Failed to serialize request".to_string())
        );

        // Send request and process response (stream_request will automatically handle XML feature)
        self.stream_request(request).await
    }

    /// Upload local file
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
            warn!("File not found: {}", file_path);
            return Err(PoeError::FileNotFound(file_path.to_string()));
        }

        // Simplify MIME type handling: use provided mime_type if available, otherwise use default
        let content_type = mime_type.unwrap_or("application/octet-stream").to_string();

        #[cfg(feature = "trace")]
        debug!("Using MIME type: {}", content_type);

        // Create multipart form
        let file = tokio::fs::File::open(path).await.map_err(|e| {
            #[cfg(feature = "trace")]
            warn!("Failed to open file: {}", e);
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
                    PoeError::FileUploadFailed(format!("Failed to set MIME type: {}", e))
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

        // Create multipart form
        let form = reqwest::multipart::Form::new().text("download_url", download_url.to_string());

        // Send request
        self.send_upload_request(form).await
    }

    /// Batch upload files (accepts mixed local and remote files)
    pub async fn upload_files_batch(
        &self,
        files: Vec<FileUploadRequest>,
    ) -> Result<Vec<FileUploadResponse>, PoeError> {
        #[cfg(feature = "trace")]
        debug!("Starting batch file upload, count: {}", files.len());

        if files.is_empty() {
            return Ok(Vec::new());
        }

        // Create upload tasks for each file
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

        for task_result in results.into_iter() {
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
                    return Err(PoeError::FileUploadFailed(format!("Upload task failed: {}", e)));
                }
            }
        }

        #[cfg(feature = "trace")]
        debug!("Batch upload successful, total {} files", upload_responses.len());

        Ok(upload_responses)
    }

    /// Send file upload request (internal method)
    async fn send_upload_request(
        &self,
        form: reqwest::multipart::Form,
    ) -> Result<FileUploadResponse, PoeError> {
        #[cfg(feature = "trace")]
        debug!("Sending file upload request to {}", self.poe_file_upload_url);

        let response = self
            .client
            .post(&self.poe_file_upload_url)
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
                .unwrap_or_else(|_| "Failed to read response content".to_string());

            #[cfg(feature = "trace")]
            warn!("File upload API response error - Status code: {}, Content: {}", status, text);

            return Err(PoeError::FileUploadFailed(format!(
                "Upload failed - Status code: {}, Content: {}",
                status, text
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

    /// Get model list for v1/models API (requires access_key)
    pub async fn get_v1_model_list(&self) -> Result<ModelResponse, PoeError> {
        #[cfg(feature = "trace")]
        debug!("Starting to get v1/models model list");

        let url = format!("{}/v1/models", self.poe_base_url);
        #[cfg(feature = "trace")]
        debug!("Sending v1/models request to URL: {}", url);

        let response = self
            .client
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.access_key))
            .header("Content-Type", "application/json")
            .send()
            .await
            .map_err(|e| {
                #[cfg(feature = "trace")]
                warn!("Failed to send v1/models request: {}", e);
                PoeError::RequestFailed(e)
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response
                .text()
                .await
                .unwrap_or_else(|_| "Failed to read response content".to_string());

            #[cfg(feature = "trace")]
            warn!(
                "v1/models API response error - Status code: {}, Content: {}",
                status, text
            );

            return Err(PoeError::BotError(format!(
                "v1/models API response error - Status code: {}, Content: {}",
                status, text
            )));
        }

        #[cfg(feature = "trace")]
        debug!("Successfully received v1/models response");

        let response_text = response.text().await.map_err(|e| {
            #[cfg(feature = "trace")]
            warn!("Failed to read v1/models response content: {}", e);
            PoeError::RequestFailed(e)
        })?;

        #[cfg(feature = "trace")]
        debug!("v1/models response content: {}", response_text);

        let json_data: Value = serde_json::from_str(&response_text).map_err(|e| {
            #[cfg(feature = "trace")]
            warn!("Failed to parse v1/models response: {}", e);
            PoeError::JsonParseFailed(e)
        })?;

        let mut model_list = Vec::new();

        if let Some(data_array) = json_data.get("data").and_then(Value::as_array) {
            #[cfg(feature = "trace")]
            debug!("Found {} models", data_array.len());

            for model_data in data_array {
                if let (Some(id), Some(object), Some(created), Some(owned_by)) = (
                    model_data.get("id").and_then(Value::as_str),
                    model_data.get("object").and_then(Value::as_str),
                    model_data.get("created").and_then(Value::as_i64),
                    model_data.get("owned_by").and_then(Value::as_str),
                ) {
                    model_list.push(ModelInfo {
                        id: id.to_string(),
                        object: object.to_string(),
                        created,
                        owned_by: owned_by.to_string(),
                    });
                }
            }
        } else {
            #[cfg(feature = "trace")]
            warn!("Could not get model list from v1/models response");
            return Err(PoeError::BotError(
                "Could not get model list from v1/models response".to_string(),
            ));
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

    /// Remove XML tool call parts from text
    #[cfg(feature = "xml")]
    pub fn remove_xml_tool_calls(text: &str) -> String {
        // Create a temporary ChatMessage to detect tool calls
        let message = ChatMessage {
            role: "assistant".to_string(),
            content: text.to_string(),
            attachments: None,
            content_type: "text/plain".to_string(),
        };

        // If no tool calls are detected, return the original text directly
        if !message.contains_xml_tool_calls() {
            return text.to_string();
        }

        // Extract tool calls to understand which parts need to be removed
        let tool_calls = message.extract_xml_tool_calls();
        if tool_calls.is_empty() {
            return text.to_string();
        }

        let mut result = text.to_string();

        // Remove <tool_call>...</tool_call> tags
        while let Some(start) = result.find("<tool_call>") {
            if let Some(end) = result[start..].find("</tool_call>") {
                let end_pos = start + end + "</tool_call>".len();
                result.replace_range(start..end_pos, "");
            } else {
                break;
            }
        }

        // Remove corresponding tool tags based on detected tool calls
        for tool_call in &tool_calls {
            let tool_name = &tool_call.function.name;
            let start_pattern = format!("<{}>", tool_name);
            let end_pattern = format!("</{}>", tool_name);

            while let Some(start) = result.find(&start_pattern) {
                if let Some(end) = result[start..].find(&end_pattern) {
                    let end_pos = start + end + end_pattern.len();
                    result.replace_range(start..end_pos, "");
                } else {
                    break;
                }
            }
        }

        // Remove <invoke> tag (if it exists)
        while let Some(start) = result.find("<invoke") {
            if let Some(end) = result[start..].find("</invoke>") {
                let end_pos = start + end + "</invoke>".len();
                result.replace_range(start..end_pos, "");
            } else {
                break;
            }
        }

        // Clean up extra empty lines
        result
            .lines()
            .filter(|line| !line.trim().is_empty())
            .collect::<Vec<_>>()
            .join("\n")
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
            warn!("Failed to build HTTP client: {}", e);
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
        HeaderValue::from_static("zh-TW,zh;q=0.9,en-US;q=0.8,en;q=0.7"),
    );
    headers.insert("Origin", HeaderValue::from_static("https://poe.com"));
    headers.insert("Referer", HeaderValue::from_static("https://poe.com"));
    headers.insert("Sec-Fetch-Dest", HeaderValue::from_static("empty"));
    headers.insert("Sec-Fetch-Mode", HeaderValue::from_static("cors"));
    headers.insert("Sec-Fetch-Site", HeaderValue::from_static("same-origin"));
    headers.insert(
        "poe-revision",
        HeaderValue::from_static(POE_GQL_MODEL_REVISION),
    );
    headers.insert("poegraphql", HeaderValue::from_static("1"));

    if let Some(code) = language_code {
        let cookie_value = format!("Poe-Language-Code={}; p-b=1", code);
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
            .unwrap_or_else(|_| "Failed to read response content".to_string());

        #[cfg(feature = "trace")]
        warn!("GraphQL API response error - Status code: {}, Content: {}", status, text);

        return Err(PoeError::BotError(format!(
            "API response error - Status code: {}, Content: {}",
            status, text
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
                debug!("Model node does not have 'handle' field");
            }
        }
    } else {
        #[cfg(feature = "trace")]
        warn!("Could not get model list nodes from response");
        return Err(PoeError::BotError("Could not get model list from response".to_string()));
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
