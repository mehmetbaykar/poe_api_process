use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ProtocolMessage {
    pub role: String,
    pub content: String,
    pub content_type: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct QueryRequest {
    pub version: String,
    pub r#type: String,
    pub query: Vec<ProtocolMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    pub user_id: String,
    pub conversation_id: String,
    pub message_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<Tool>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_results: Option<Vec<ToolResult>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Tool {
    pub r#type: String,
    pub function: ToolFunction,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ToolFunction {
    pub name: String,
    pub description: String,
    pub parameters: ToolFunctionParameters,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ToolFunctionParameters {
    pub r#type: String,
    pub properties: Value,
    pub required: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ToolCall {
    pub id: String,
    pub r#type: String,
    pub function: ToolCallFunction,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ToolCallFunction {
    pub name: String,
    pub arguments: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ToolResult {
    pub role: String,
    pub tool_call_id: String,
    pub name: String,
    pub content: String,
}

#[derive(Debug, Clone, Default)]
pub struct AccumulatedToolCall {
    pub id: String,
    pub r#type: String,
    pub function_name: String,
    pub function_arguments: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PartialResponse {
    pub text: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ErrorResponse {
    pub text: String,
    pub allow_retry: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ModelInfo {
    pub id: String,
    pub object: String,
    pub created: i64,
    pub owned_by: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ModelListResponse {
    pub data: Vec<ModelInfo>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum EventType {
    Text,
    ReplaceResponse,
    Json,
    Done,
    Error,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EventResponse {
    pub event: EventType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<PartialResponse>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ErrorResponse>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
}
