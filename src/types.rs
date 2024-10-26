use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct ProtocolMessage {
    pub role: String,
    pub content: String,
    pub content_type: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct QueryRequest {
    pub version: String,
    pub r#type: String,
    pub query: Vec<ProtocolMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    pub user_id: String,
    pub conversation_id: String,
    pub message_id: String,
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
}