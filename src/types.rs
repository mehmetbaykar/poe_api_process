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
    #[serde(default = "default_tool_type")]
    pub r#type: String,
    pub function: FunctionDefinition,
}

fn default_tool_type() -> String {
    "function".to_string()
}

// ChatTool 的FunctionDefinition 結構
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FunctionDefinition {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parameters: Option<FunctionParameters>,
}

// FunctionDefinition 的FunctionParameters 結構
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FunctionParameters {
    #[serde(default = "default_parameters_type")]
    pub r#type: String,
    pub properties: Value,
    #[serde(default)]
    pub required: Vec<String>,
}

fn default_parameters_type() -> String {
    "object".to_string()
}

// 工具呼叫相關結構
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ChatToolCall {
    pub id: String,
    #[serde(default = "default_tool_call_type")]
    pub r#type: String,
    pub function: FunctionCall,
}

fn default_tool_call_type() -> String {
    "function".to_string()
}

// ChatToolCall 的FunctionCall 結構
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FunctionCall {
    pub name: String,
    #[serde(default)]
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
    LocalFile {
        file: String,
        mime_type: Option<String>,
    },
    RemoteFile {
        download_url: String,
    },
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json;

    #[test]
    fn test_tool_deserialization_missing_type_field() {
        // Test tool without type field (should default to "function")
        let tool_json = r#"{
            "function": {
                "name": "get_weather",
                "description": "Get the current weather",
                "parameters": {
                    "properties": {
                        "location": {
                            "type": "string",
                            "description": "The city name"
                        }
                    }
                }
            }
        }"#;

        let result: Result<ChatTool, _> = serde_json::from_str(tool_json);
        assert!(result.is_ok(), "Tool without type field should deserialize successfully");

        let tool = result.unwrap();
        assert_eq!(tool.r#type, "function");
        assert_eq!(tool.function.name, "get_weather");
        assert_eq!(tool.function.description.as_deref(), Some("Get the current weather"));

        if let Some(params) = &tool.function.parameters {
            assert_eq!(params.r#type, "object");
            assert_eq!(params.required.len(), 0); // Should default to empty vec
        }
    }

    #[test]
    fn test_tool_deserialization_missing_required_field() {
        // Test parameters without required field (should default to empty vec)
        let params_json = r#"{
            "type": "object",
            "properties": {
                "location": {
                    "type": "string",
                    "description": "The city name"
                }
            }
        }"#;

        let result: Result<FunctionParameters, _> = serde_json::from_str(params_json);
        assert!(result.is_ok(), "FunctionParameters without required field should deserialize successfully");

        let params = result.unwrap();
        assert_eq!(params.r#type, "object");
        assert_eq!(params.required.len(), 0); // Should default to empty vec
    }

    #[test]
    fn test_tool_call_deserialization_missing_type_field() {
        // Test tool call without type field (should default to "function")
        let tool_call_json = r#"{
            "id": "call_123",
            "function": {
                "name": "get_weather",
                "arguments": "{\"location\": \"New York\"}"
            }
        }"#;

        let result: Result<ChatToolCall, _> = serde_json::from_str(tool_call_json);
        assert!(result.is_ok(), "ChatToolCall without type field should deserialize successfully");

        let tool_call = result.unwrap();
        assert_eq!(tool_call.r#type, "function");
        assert_eq!(tool_call.id, "call_123");
        assert_eq!(tool_call.function.name, "get_weather");
    }

    #[test]
    fn test_function_call_missing_arguments_field() {
        // Test function call without arguments field (should default to empty string)
        let func_call_json = r#"{
            "name": "get_weather"
        }"#;

        let result: Result<FunctionCall, _> = serde_json::from_str(func_call_json);
        assert!(result.is_ok(), "FunctionCall without arguments field should deserialize successfully");

        let func_call = result.unwrap();
        assert_eq!(func_call.name, "get_weather");
        assert_eq!(func_call.arguments, ""); // Should default to empty string
    }
}
