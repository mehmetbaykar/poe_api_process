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
pub struct ModelInfo {
    pub id: String,
    pub object: String
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ModelListResponse {
    pub data: Vec<ModelInfo>,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum EventType {
    Text,
    ReplaceResponse,
    Done,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EventResponse {
    pub event: EventType,
    pub data: Option<PartialResponse>,
}