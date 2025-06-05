use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

// Bot Chat 請求結構
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

// 消息結構
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attachments: Option<Vec<Attachment>>,
    pub content_type: String,
}

// ChatMessage 的Attachment 結構
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Attachment {
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_type: Option<String>,
}

// 工具定義相關結構
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ChatTool {
    pub r#type: String,
    pub function: FunctionDefinition,
}

// ChatTool 的FunctionDefinition 結構
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FunctionDefinition {
    pub name: String,
    pub description: String,
    pub parameters: FunctionParameters,
}

// FunctionDefinition 的FunctionParameters 結構
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FunctionParameters {
    pub r#type: String,
    pub properties: Value,
    pub required: Vec<String>,
}

// 工具呼叫相關結構
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ChatToolCall {
    pub id: String,
    pub r#type: String,
    pub function: FunctionCall,
}

// ChatToolCall 的FunctionCall 結構
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FunctionCall {
    pub name: String,
    pub arguments: String,
}

// 工具呼叫結果
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ChatToolResult {
    pub role: String,
    pub tool_call_id: String,
    pub name: String,
    pub content: String,
}

// 用於追蹤部分工具呼叫
#[derive(Debug, Clone, Default)]
pub struct PartialToolCall {
    pub id: String,
    pub r#type: String,
    pub function_name: String,
    pub function_arguments: String,
}

// 事件響應
#[derive(Debug, Serialize, Deserialize)]
pub struct ChatResponse {
    pub event: ChatEventType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<ChatResponseData>,
}

// 事件類型
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub enum ChatEventType {
    Text,
    ReplaceResponse,
    Json,
    File,
    Done,
    Error,
}

// 檔案數據結構
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FileData {
    pub url: String,
    pub name: String,
    pub content_type: String,
    pub inline_ref: String,
}

// 響應資料的可能類型
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

// 模型信息
#[derive(Debug, Serialize, Deserialize)]
pub struct ModelInfo {
    pub id: String,
    pub object: String,
    pub created: i64,
    pub owned_by: String,
}

// 文件上傳請求結構
#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum FileUploadRequest {
    LocalFile { file: String },
    RemoteFile { download_url: String },
}

// 文件上傳響應結構
#[derive(Debug, Serialize, Deserialize)]
pub struct FileUploadResponse {
    pub attachment_url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<u64>,
}
