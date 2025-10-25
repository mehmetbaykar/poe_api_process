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
    #[serde(default = "default_chat_tool_type")]
    pub r#type: String,
    #[serde(default)]
    pub function: FunctionDefinition,
    #[serde(flatten, default)]
    pub extra: HashMap<String, Value>,
}

fn default_chat_tool_type() -> String {
    "function".to_string()
}

impl Default for ChatTool {
    fn default() -> Self {
        Self {
            r#type: default_chat_tool_type(),
            function: FunctionDefinition::default(),
            extra: HashMap::new(),
        }
    }
}

// FunctionDefinition structure for ChatTool
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct FunctionDefinition {
    #[serde(default)]
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parameters: Option<FunctionParameters>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub returns: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub strict: Option<bool>,
    #[serde(flatten, default)]
    pub extra: HashMap<String, Value>,
}

// FunctionParameters structure for FunctionDefinition
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(default)]
pub struct FunctionParameters {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub r#type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub properties: Option<Value>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub required: Vec<String>,
    #[serde(flatten, default)]
    pub extra: HashMap<String, Value>,
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn function_parameters_accept_optional_fields() {
        let value = json!({
            "type": "object",
            "properties": {
                "city": { "type": "string" }
            },
            "additionalProperties": false
        });

        let params: FunctionParameters = serde_json::from_value(value).unwrap();
        assert_eq!(params.r#type.as_deref(), Some("object"));
        assert!(params.required.is_empty());
        assert!(params.properties.is_some());
        assert_eq!(
            params
                .extra
                .get("additionalProperties")
                .and_then(|v| v.as_bool()),
            Some(false)
        );
    }

    #[test]
    fn chat_tool_defaults_type_and_preserves_extras() {
        let raw = json!({
            "function": {
                "name": "lookup",
                "strict": true,
                "returns": { "type": "string" },
                "parameters": {
                    "properties": {
                        "query": { "type": "string" }
                    },
                    "dependentRequired": {
                        "query": ["format"]
                    }
                },
                "x-extra-field": "value"
            },
            "metadata": "tool-meta"
        });

        let tool: ChatTool = serde_json::from_value(raw).unwrap();
        assert_eq!(tool.r#type, "function");
        assert_eq!(
            tool.extra.get("metadata").and_then(|v| v.as_str()),
            Some("tool-meta")
        );

        let function = tool.function;
        assert_eq!(function.name, "lookup");
        assert_eq!(function.strict, Some(true));
        assert!(function.returns.is_some());
        assert_eq!(
            function.extra.get("x-extra-field").and_then(|v| v.as_str()),
            Some("value")
        );

        let parameters = function.parameters.unwrap();
        assert!(parameters.required.is_empty());
        assert!(parameters.properties.is_some());
        assert!(
            parameters.extra.get("dependentRequired").is_some(),
            "dependentRequired should be preserved in extra map"
        );
    }
}
