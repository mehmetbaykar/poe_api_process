use crate::types::*;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

/// Logging configuration for request/response logging
#[derive(Debug, Clone)]
pub struct LoggingConfig {
    pub log_requests: bool,
    pub log_responses: bool,
    pub log_headers: bool,
    pub log_body: bool,
    pub max_body_length: usize,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            log_requests: true,
            log_responses: true,
            log_headers: true,
            log_body: true,
            max_body_length: 10000, // 10KB max body length
        }
    }
}

/// Request logging information
#[derive(Debug, Clone)]
pub struct RequestLog {
    pub timestamp: u64,
    pub method: String,
    pub url: String,
    pub headers: Option<Vec<(String, String)>>,
    pub body: Option<String>,
    pub body_size: Option<usize>,
}

/// Response logging information
#[derive(Debug, Clone)]
pub struct ResponseLog {
    pub timestamp: u64,
    pub status_code: u16,
    pub headers: Option<Vec<(String, String)>>,
    pub body: Option<String>,
    pub body_size: Option<usize>,
    pub duration_ms: Option<u64>,
}

/// Logging helper functions
pub struct LoggingHelper;

impl LoggingHelper {
    /// Get current timestamp in milliseconds
    pub fn get_timestamp() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64
    }

    /// Format request log for display
    pub fn format_request_log(log: &RequestLog, config: &LoggingConfig) -> String {
        let mut output = String::new();
        
        output.push_str(&format!("📤 OUTGOING REQUEST [{}]\n", log.timestamp));
        output.push_str(&format!("   Method: {}\n", log.method));
        output.push_str(&format!("   URL: {}\n", log.url));
        
        if config.log_headers {
            if let Some(headers) = &log.headers {
                output.push_str("   Headers:\n");
                for (key, value) in headers {
                    // Mask sensitive headers
                    let masked_value = if key.to_lowercase().contains("authorization") {
                        "***MASKED***".to_string()
                    } else {
                        value.clone()
                    };
                    output.push_str(&format!("     {}: {}\n", key, masked_value));
                }
            }
        }
        
        if config.log_body {
            if let Some(body) = &log.body {
                let truncated_body = if body.len() > config.max_body_length {
                    format!("{}... [truncated, {} bytes total]", 
                           &body[..config.max_body_length], body.len())
                } else {
                    body.clone()
                };
                output.push_str(&format!("   Body ({} bytes):\n", log.body_size.unwrap_or(0)));
                output.push_str(&format!("     {}\n", truncated_body));
            }
        }
        
        output
    }

    /// Format response log for display
    pub fn format_response_log(log: &ResponseLog, config: &LoggingConfig) -> String {
        let mut output = String::new();
        
        let status_emoji = match log.status_code {
            200..=299 => "✅",
            300..=399 => "🔄",
            400..=499 => "❌",
            500..=599 => "💥",
            _ => "❓",
        };
        
        output.push_str(&format!("📥 INCOMING RESPONSE [{}] {} {}\n", 
                                log.timestamp, status_emoji, log.status_code));
        
        if let Some(duration) = log.duration_ms {
            output.push_str(&format!("   Duration: {}ms\n", duration));
        }
        
        if config.log_headers {
            if let Some(headers) = &log.headers {
                output.push_str("   Headers:\n");
                for (key, value) in headers {
                    output.push_str(&format!("     {}: {}\n", key, value));
                }
            }
        }
        
        if config.log_body {
            if let Some(body) = &log.body {
                let truncated_body = if body.len() > config.max_body_length {
                    format!("{}... [truncated, {} bytes total]", 
                           &body[..config.max_body_length], body.len())
                } else {
                    body.clone()
                };
                output.push_str(&format!("   Body ({} bytes):\n", log.body_size.unwrap_or(0)));
                output.push_str(&format!("     {}\n", truncated_body));
            }
        }
        
        output
    }

    /// Format chat request for logging
    pub fn format_chat_request(request: &ChatRequest) -> String {
        let mut output = String::new();
        
        output.push_str("🤖 CHAT REQUEST:\n");
        output.push_str(&format!("   Version: {}\n", request.version));
        output.push_str(&format!("   Type: {}\n", request.r#type));
        output.push_str(&format!("   Messages: {} message(s)\n", request.query.len()));
        
        for (i, message) in request.query.iter().enumerate() {
            output.push_str(&format!("   Message {}: {} ({} chars)\n", 
                                   i + 1, message.role, message.content.len()));
        }
        
        if let Some(tools) = &request.tools {
            output.push_str(&format!("   Tools: {} tool(s)\n", tools.len()));
        }
        
        if let Some(tool_calls) = &request.tool_calls {
            output.push_str(&format!("   Tool Calls: {} call(s)\n", tool_calls.len()));
        }
        
        if let Some(tool_results) = &request.tool_results {
            output.push_str(&format!("   Tool Results: {} result(s)\n", tool_results.len()));
        }
        
        if let Some(temperature) = &request.temperature {
            output.push_str(&format!("   Temperature: {}\n", temperature));
        }
        
        output
    }

    /// Format chat response for logging
    pub fn format_chat_response(response: &ChatResponse) -> String {
        let mut output = String::new();
        
        output.push_str("🤖 CHAT RESPONSE:\n");
        output.push_str(&format!("   Event: {:?}\n", response.event));
        
        match &response.data {
            Some(ChatResponseData::Text { text }) => {
                output.push_str(&format!("   Text: {} chars\n", text.len()));
            }
            Some(ChatResponseData::ToolCalls(tool_calls)) => {
                output.push_str(&format!("   Tool Calls: {} call(s)\n", tool_calls.len()));
                for (i, call) in tool_calls.iter().enumerate() {
                    output.push_str(&format!("     Call {}: {} ({} chars)\n", 
                                           i + 1, call.function.name, call.function.arguments.len()));
                }
            }
            Some(ChatResponseData::Error { text, allow_retry }) => {
                output.push_str(&format!("   Error: {} (retry: {})\n", text, allow_retry));
            }
            Some(ChatResponseData::File(file_data)) => {
                output.push_str(&format!("   File: {} ({})\n", file_data.name, file_data.content_type));
            }
            Some(ChatResponseData::Empty) => {
                output.push_str("   Status: Empty\n");
            }
            None => {
                output.push_str("   Status: No data\n");
            }
        }
        
        output
    }

    /// Format error for logging
    pub fn format_error(error: &crate::error::PoeError) -> String {
        let mut output = String::new();
        
        output.push_str("💥 ERROR:\n");
        output.push_str(&format!("   Type: {:?}\n", error));
        output.push_str(&format!("   Message: {}\n", error));
        
        output
    }
}

#[cfg(feature = "trace")]
use tracing::{debug, error};

#[cfg(feature = "trace")]
impl LoggingHelper {
    /// Log request with tracing
    pub fn log_request(log: &RequestLog, config: &LoggingConfig) {
        let formatted = Self::format_request_log(log, config);
        debug!("{}", formatted);
    }

    /// Log response with tracing
    pub fn log_response(log: &ResponseLog, config: &LoggingConfig) {
        let formatted = Self::format_response_log(log, config);
        debug!("{}", formatted);
    }

    /// Log chat request with tracing
    pub fn log_chat_request(request: &ChatRequest) {
        let formatted = Self::format_chat_request(request);
        debug!("{}", formatted);
    }

    /// Log chat response with tracing
    pub fn log_chat_response(response: &ChatResponse) {
        let formatted = Self::format_chat_response(response);
        debug!("{}", formatted);
    }

    /// Log error with tracing
    pub fn log_error(error: &crate::error::PoeError) {
        let formatted = Self::format_error(error);
        error!("{}", formatted);
    }
}
