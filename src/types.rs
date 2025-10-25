use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

// Bot Chat Request structure
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ChatRequest {
    pub version: String,
    pub r#type: String,
    pub query: Vec<ChatMessage>,
    pub user_id: String,
    pub conversation_id: String,
    pub message_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<ChatTool>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ChatToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_results: Option<Vec<ChatToolResult>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logit_bias: Option<HashMap<String, f32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_sequences: Option<Vec<String>>,
}

// Message structure
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attachments: Option<Vec<Attachment>>,
    pub content_type: String,
}

// Attachment structure for ChatMessage
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Attachment {
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_type: Option<String>,
}

// Tool definition related structure
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ChatTool {
    pub r#type: String,
    pub function: FunctionDefinition,
}

// FunctionDefinition structure for ChatTool
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FunctionDefinition {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parameters: Option<FunctionParameters>,
}

// FunctionParameters structure for FunctionDefinition
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FunctionParameters {
    pub r#type: String,
    pub properties: Value,
    pub required: Vec<String>,
}

// Tool call related structure
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ChatToolCall {
    pub id: String,
    pub r#type: String,
    pub function: FunctionCall,
}

// FunctionCall structure for ChatToolCall
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FunctionCall {
    pub name: String,
    pub arguments: String,
}

// Tool call result
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ChatToolResult {
    pub role: String,
    pub tool_call_id: String,
    pub name: String,
    pub content: String,
}

// Used to track partial tool calls
#[derive(Debug, Clone, Default)]
pub struct PartialToolCall {
    pub id: String,
    pub r#type: String,
    pub function_name: String,
    pub function_arguments: String,
}

// Event response
#[derive(Debug, Serialize, Deserialize)]
pub struct ChatResponse {
    pub event: ChatEventType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<ChatResponseData>,
}

// Event type
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub enum ChatEventType {
    Text,
    ReplaceResponse,
    Json,
    File,
    Done,
    Error,
}

// File data structure
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FileData {
    pub url: String,
    pub name: String,
    pub content_type: String,
    pub inline_ref: String,
}

// Possible types of response data
#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ChatResponseData {
    Text { text: String },
    Error { text: String, allow_retry: bool },
    ToolCalls(Vec<ChatToolCall>),
    File(FileData),
    Empty,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ModelResponse {
    pub data: Vec<ModelInfo>,
}

// Model information
#[derive(Debug, Serialize, Deserialize)]
pub struct ModelInfo {
    pub id: String,
    pub object: String,
    pub created: i64,
    pub owned_by: String,
}

// File upload request structure
#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum FileUploadRequest {
    LocalFile {
        file: String,
        mime_type: Option<String>,
    },
    RemoteFile {
        download_url: String,
    },
}

// File upload response structure
#[derive(Debug, Serialize, Deserialize)]
pub struct FileUploadResponse {
    pub attachment_url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<u64>,
}
