use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct ProtocolMessage {
    pub role: String,
    pub content: String,
    pub content_type: Option<String>,
    pub timestamp: Option<i64>,
    pub message_id: Option<String>,
    pub feedback: Option<Vec<MessageFeedback>>,
    pub attachments: Option<Vec<Attachment>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MessageFeedback {
    pub feedback_type: String,
    pub reason: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Attachment {
    pub url: String,
    pub content_type: String,
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct QueryRequest {
    pub version: String,
    pub r#type: String,
    pub query: Vec<ProtocolMessage>,
    pub user_id: String,
    pub conversation_id: String,
    pub message_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SettingsResponse {
    pub server_bot_dependencies: Option<std::collections::HashMap<String, i32>>,
    pub allow_attachments: Option<bool>,
    pub introduction_message: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PartialResponse {
    pub text: String,
    pub is_suggested_reply: bool,
    pub is_replace_response: bool,
}