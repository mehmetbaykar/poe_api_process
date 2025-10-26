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

    #[test]
    fn test_tool_call_serialization() {
        let tool_call = ChatToolCall {
            id: "call_abc123".to_string(),
            r#type: "function".to_string(),
            function: FunctionCall {
                name: "get_weather".to_string(),
                arguments: r#"{"city": "London"}"#.to_string(),
            },
        };

        let serialized = serde_json::to_value(&tool_call).unwrap();
        assert_eq!(serialized["id"], "call_abc123");
        assert_eq!(serialized["type"], "function");
        assert_eq!(serialized["function"]["name"], "get_weather");

        // Test deserialization
        let deserialized: ChatToolCall = serde_json::from_value(serialized).unwrap();
        assert_eq!(deserialized.id, "call_abc123");
        assert_eq!(deserialized.function.name, "get_weather");
    }

    #[test]
    fn test_tool_result_structure() {
        let tool_result = ChatToolResult {
            role: "tool".to_string(),
            tool_call_id: "call_abc123".to_string(),
            name: "get_weather".to_string(),
            content: r#"{"temperature": 20, "condition": "sunny"}"#.to_string(),
        };

        let serialized = serde_json::to_value(&tool_result).unwrap();
        assert_eq!(serialized["role"], "tool");
        assert_eq!(serialized["tool_call_id"], "call_abc123");
        assert_eq!(serialized["name"], "get_weather");
        assert!(serialized["content"].is_string());

        // Test deserialization
        let deserialized: ChatToolResult = serde_json::from_value(serialized).unwrap();
        assert_eq!(deserialized.tool_call_id, "call_abc123");
        assert_eq!(deserialized.name, "get_weather");
    }

    #[test]
    fn test_chat_request_with_tool_results() {
        let chat_request = ChatRequest {
            version: "1.1".to_string(),
            r#type: "query".to_string(),
            query: vec![ChatMessage {
                role: "user".to_string(),
                content: "What's the weather?".to_string(),
                attachments: None,
                content_type: "text/plain".to_string(),
            }],
            user_id: "user123".to_string(),
            conversation_id: "conv123".to_string(),
            message_id: "msg123".to_string(),
            tools: None,
            tool_calls: None,
            tool_results: Some(vec![ChatToolResult {
                role: "tool".to_string(),
                tool_call_id: "call_abc123".to_string(),
                name: "get_weather".to_string(),
                content: r#"{"temperature": 20}"#.to_string(),
            }]),
            temperature: None,
            logit_bias: None,
            stop_sequences: None,
        };

        let serialized = serde_json::to_value(&chat_request).unwrap();
        assert!(serialized["tool_results"].is_array());
        assert_eq!(serialized["tool_results"][0]["tool_call_id"], "call_abc123");

        // Verify tool_results field is included in serialization
        let json_str = serde_json::to_string(&chat_request).unwrap();
        assert!(json_str.contains("tool_results"));
        assert!(json_str.contains("call_abc123"));
    }

    #[test]
    fn test_cursor_style_tool_definition() {
        // Test tool definition in Cursor's format
        let cursor_tool = json!({
            "type": "function",
            "function": {
                "name": "execute_command",
                "description": "Execute a shell command",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "command": {
                            "type": "string",
                            "description": "The command to execute"
                        }
                    },
                    "required": ["command"]
                }
            }
        });

        let tool: ChatTool = serde_json::from_value(cursor_tool).unwrap();
        assert_eq!(tool.r#type, "function");
        assert_eq!(tool.function.name, "execute_command");

        let params = tool.function.parameters.unwrap();
        assert_eq!(params.required, vec!["command"]);
        assert!(params.properties.is_some());
    }

    #[test]
    fn test_empty_required_array_in_parameters() {
        // Cursor sometimes sends tools without required field
        let tool_json = json!({
            "type": "function",
            "function": {
                "name": "optional_tool",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "param1": { "type": "string" }
                    }
                }
            }
        });

        let tool: ChatTool = serde_json::from_value(tool_json).unwrap();
        let params = tool.function.parameters.unwrap();
        assert!(
            params.required.is_empty(),
            "Required should default to empty vec"
        );
    }
}
