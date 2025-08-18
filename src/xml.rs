use crate::types::{ChatMessage, ChatRequest, ChatTool, ChatToolCall, FunctionCall};
use std::collections::HashMap;

// XML 工具格式相關結構
#[derive(Debug, Clone)]
pub struct XmlTool {
    pub name: String,
    pub description: Option<String>,
    pub parameters: Vec<XmlParameter>,
}

#[derive(Debug, Clone)]
pub struct XmlParameter {
    pub name: String,
    pub param_type: String,
    pub description: Option<String>,
    pub required: bool,
    pub enum_values: Option<Vec<String>>,
}

// XML 工具轉換 trait
pub trait ToXml {
    fn to_xml(&self) -> String;
}

impl ToXml for ChatTool {
    fn to_xml(&self) -> String {
        let mut xml = String::new();
        xml.push_str(&format!("<{}>", self.function.name));

        if let Some(ref description) = self.function.description {
            xml.push_str(&format!(
                "\n<description>{}</description>",
                escape_xml(description)
            ));
        }

        if let Some(ref parameters) = self.function.parameters {
            xml.push_str("\n<parameters>");

            if let Some(properties) = parameters.properties.as_object() {
                for (param_name, param_value) in properties {
                    xml.push_str(&format!(
                        "\n<{}_name>{}</{}_name>",
                        param_name, param_name, param_name
                    ));

                    if let Some(param_type) = param_value.get("type").and_then(|v| v.as_str()) {
                        xml.push_str(&format!(
                            "\n<{}_type>{}</{}_type>",
                            param_name, param_type, param_name
                        ));
                    }

                    if let Some(param_desc) =
                        param_value.get("description").and_then(|v| v.as_str())
                    {
                        xml.push_str(&format!(
                            "\n<{}_description>{}</{}_description>",
                            param_name,
                            escape_xml(param_desc),
                            param_name
                        ));
                    }

                    let is_required = parameters.required.contains(param_name);
                    xml.push_str(&format!(
                        "\n<{}_required>{}</{}_required>",
                        param_name, is_required, param_name
                    ));

                    if let Some(enum_values) = param_value.get("enum").and_then(|v| v.as_array()) {
                        xml.push_str(&format!("\n<{}_enum>", param_name));
                        for enum_val in enum_values {
                            if let Some(val_str) = enum_val.as_str() {
                                xml.push_str(&format!(
                                    "\n<option>{}</option>",
                                    escape_xml(val_str)
                                ));
                            }
                        }
                        xml.push_str(&format!("\n</{}_enum>", param_name));
                    }
                }
            }

            xml.push_str("\n</parameters>");
        }

        xml.push_str(&format!("\n</{}>", self.function.name));
        xml
    }
}

impl ToXml for Vec<ChatTool> {
    fn to_xml(&self) -> String {
        if self.is_empty() {
            return String::new();
        }

        let mut xml = String::from("\n\n<tools>");
        for tool in self {
            xml.push('\n');
            xml.push_str(&tool.to_xml());
        }
        xml.push_str("\n</tools>");
        xml
    }
}

// XML 轉義函數
fn escape_xml(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

// 為 ChatMessage 添加 XML 工具附加功能（僅內部使用）
impl ChatMessage {
    /// 將 XML 格式的工具定義附加到消息內容末尾（內部使用）
    pub(crate) fn append_xml_tools(&mut self, tools: &[ChatTool]) {
        if !tools.is_empty() {
            let tools_vec = tools.to_vec();
            let xml_tools = tools_vec.to_xml();
            self.content.push_str(&xml_tools);
        }
    }
}

// 為 ChatRequest 添加 XML 工具處理功能（僅內部使用）
impl ChatRequest {
    /// 將工具轉換為 XML 格式並附加到最後一條用戶消息中（內部使用）
    pub(crate) fn append_tools_as_xml(&mut self) {
        if let Some(ref tools) = self.tools {
            if !tools.is_empty() {
                // 找到最後一條用戶消息
                for message in self.query.iter_mut().rev() {
                    if message.role == "user" {
                        message.append_xml_tools(tools);
                        break;
                    }
                }
            }
        }
    }
}

// XML 工具調用解析功能
pub struct XmlToolCallParser;

impl XmlToolCallParser {
    /// 從文本中解析 XML 工具調用
    pub fn parse_xml_tool_calls(text: &str) -> Vec<ChatToolCall> {
        let mut tool_calls = Vec::new();

        // 查找所有的工具調用標籤
        if let Some(start) = text.find("<tool_call>") {
            let mut current_pos = start;
            let mut call_id = 1;

            while let Some(call_start) = text[current_pos..].find("<tool_call>") {
                let actual_start = current_pos + call_start;

                if let Some(call_end) = text[actual_start..].find("</tool_call>") {
                    let actual_end = actual_start + call_end + "</tool_call>".len();
                    let call_content = &text[actual_start..actual_end];

                    if let Some(tool_call) = Self::parse_single_tool_call(call_content, call_id) {
                        tool_calls.push(tool_call);
                        call_id += 1;
                    }

                    current_pos = actual_end;
                } else {
                    break;
                }
            }
        }

        tool_calls
    }

    /// 基於提供的工具定義從文本中解析 XML 工具調用
    pub fn parse_xml_tool_calls_with_tools(text: &str, tools: &[ChatTool]) -> Vec<ChatToolCall> {
        let mut tool_calls = Vec::new();

        // 首先嘗試標準格式
        tool_calls.extend(Self::parse_xml_tool_calls(text));

        // 如果沒有找到標準格式的工具調用，嘗試基於工具定義的解析
        if tool_calls.is_empty() {
            tool_calls.extend(Self::parse_tool_specific_xml_format(text, tools));
        }

        tool_calls
    }

    /// 解析單個工具調用
    fn parse_single_tool_call(xml_content: &str, call_id: u32) -> Option<ChatToolCall> {
        // 首先嘗試解析 <invoke name="tool_name"> 格式
        if let Some(function_name) = Self::extract_invoke_name(xml_content) {
            let arguments = Self::extract_parameters_as_json(xml_content);

            return Some(ChatToolCall {
                id: format!("call_{}", call_id),
                r#type: "function".to_string(),
                function: FunctionCall {
                    name: function_name,
                    arguments,
                },
            });
        }

        // 如果沒有找到 invoke 標籤，嘗試舊格式
        if let Some(function_name) = Self::extract_xml_value(xml_content, "name") {
            let arguments = Self::extract_xml_value(xml_content, "arguments")
                .unwrap_or_else(|| Self::extract_parameters_as_json(xml_content));

            return Some(ChatToolCall {
                id: format!("call_{}", call_id),
                r#type: "function".to_string(),
                function: FunctionCall {
                    name: function_name,
                    arguments,
                },
            });
        }

        None
    }

    /// 基於提供的工具定義解析 XML 格式
    fn parse_tool_specific_xml_format(text: &str, tools: &[ChatTool]) -> Vec<ChatToolCall> {
        let mut tool_calls = Vec::new();
        let mut call_id = 1;

        for tool in tools {
            if let Some(tool_call) = Self::parse_tool_tag(text, &tool.function.name, call_id) {
                tool_calls.push(tool_call);
                call_id += 1;
            }
        }

        tool_calls
    }

    /// 解析特定工具標籤
    fn parse_tool_tag(text: &str, tool_name: &str, call_id: u32) -> Option<ChatToolCall> {
        let start_tag = format!("<{}>", tool_name);
        let end_tag = format!("</{}>", tool_name);

        if let Some(start_pos) = text.find(&start_tag) {
            let content_start = start_pos + start_tag.len();
            if let Some(end_pos) = text[content_start..].find(&end_tag) {
                let tool_content = &text[content_start..content_start + end_pos];
                let arguments = Self::extract_parameters_as_json(tool_content);

                return Some(ChatToolCall {
                    id: format!("call_{}", call_id),
                    r#type: "function".to_string(),
                    function: FunctionCall {
                        name: tool_name.to_string(),
                        arguments,
                    },
                });
            }
        }

        None
    }

    /// 從 XML 中提取指定標籤的值
    fn extract_xml_value(xml: &str, tag: &str) -> Option<String> {
        let start_tag = format!("<{}>", tag);
        let end_tag = format!("</{}>", tag);

        if let Some(start) = xml.find(&start_tag) {
            let content_start = start + start_tag.len();
            if let Some(end) = xml[content_start..].find(&end_tag) {
                return Some(xml[content_start..content_start + end].trim().to_string());
            }
        }
        None
    }

    /// 從 <invoke name="tool_name"> 格式中提取工具名稱
    fn extract_invoke_name(xml: &str) -> Option<String> {
        // 查找 <invoke name="..."> 模式
        if let Some(invoke_start) = xml.find("<invoke") {
            let invoke_content = &xml[invoke_start..];
            if let Some(name_start) = invoke_content.find("name=\"") {
                let name_content_start = invoke_start + name_start + 6; // 6 = len("name=\"")
                if let Some(name_end) = xml[name_content_start..].find('"') {
                    return Some(
                        xml[name_content_start..name_content_start + name_end].to_string(),
                    );
                }
            }
        }
        None
    }

    /// 將 XML 參數轉換為 JSON 格式
    fn extract_parameters_as_json(xml_content: &str) -> String {
        let mut params = HashMap::new();

        // 首先嘗試解析 <parameter name="key">value</parameter> 格式
        let mut current_pos = 0;
        while let Some(param_start) = xml_content[current_pos..].find("<parameter") {
            let actual_start = current_pos + param_start;

            // 提取參數名
            if let Some(name_start) = xml_content[actual_start..].find("name=\"") {
                let name_content_start = actual_start + name_start + 6; // 6 = len("name=\"")
                if let Some(name_end) = xml_content[name_content_start..].find('"') {
                    let param_name =
                        xml_content[name_content_start..name_content_start + name_end].to_string();

                    // 找到參數值
                    if let Some(value_start) =
                        xml_content[name_content_start + name_end..].find('>')
                    {
                        let value_content_start = name_content_start + name_end + value_start + 1;
                        if let Some(value_end) =
                            xml_content[value_content_start..].find("</parameter>")
                        {
                            let param_value = xml_content
                                [value_content_start..value_content_start + value_end]
                                .trim();
                            if !param_value.is_empty() {
                                // 解碼 XML 實體
                                let decoded_value = Self::decode_xml_entities(param_value);
                                params.insert(param_name, decoded_value);
                            }
                        }
                    }
                }
            }

            current_pos = actual_start + 1;
        }
        // 如果沒有找到 parameter 標籤，嘗試舊格式
        if params.is_empty() {
            // 使用更強大的解析方法來處理多行 XML
            let mut current_pos = 0;
            while current_pos < xml_content.len() {
                if let Some(tag_start) = xml_content[current_pos..].find('<') {
                    let actual_start = current_pos + tag_start;

                    // 跳過結束標籤和特殊標籤
                    if xml_content[actual_start..].starts_with("</")
                        || xml_content[actual_start..].starts_with("<invoke")
                        || xml_content[actual_start..].starts_with("<parameter")
                        || xml_content[actual_start..].starts_with("<tool_call")
                    {
                        current_pos = actual_start + 1;
                        continue;
                    }

                    // 找到標籤結束
                    if let Some(tag_end) = xml_content[actual_start + 1..].find('>') {
                        let tag_name = &xml_content[actual_start + 1..actual_start + 1 + tag_end];
                        let content_start = actual_start + 1 + tag_end + 1;

                        // 找到對應的結束標籤
                        let end_tag = format!("</{}>", tag_name);
                        if let Some(end_pos) = xml_content[content_start..].find(&end_tag) {
                            let value = xml_content[content_start..content_start + end_pos].trim();
                            if !value.is_empty() {
                                let decoded_value = Self::decode_xml_entities(value);
                                params.insert(tag_name.to_string(), decoded_value);
                            }
                            current_pos = content_start + end_pos + end_tag.len();
                        } else {
                            current_pos = actual_start + 1;
                        }
                    } else {
                        current_pos = actual_start + 1;
                    }
                } else {
                    break;
                }
            }
        }

        // 轉換為 JSON
        if params.is_empty() {
            "{}".to_string()
        } else {
            serde_json::to_string(&params).unwrap_or_else(|_| "{}".to_string())
        }
    }

    /// 解碼 XML 實體
    fn decode_xml_entities(text: &str) -> String {
        text.replace("&lt;", "<")
            .replace("&gt;", ">")
            .replace("&amp;", "&")
            .replace("&quot;", "\"")
            .replace("&apos;", "'")
    }
}
// 為 ChatMessage 添加 XML 工具調用檢測功能
impl ChatMessage {
    /// 檢測消息中是否包含 XML 工具調用（通用格式）
    pub fn contains_xml_tool_calls(&self) -> bool {
        // 檢測標準的 <tool_call> 格式
        if self.content.contains("<tool_call>") {
            return true;
        }

        // 檢測 <invoke> 格式
        if self.content.contains("<invoke") {
            return true;
        }

        false
    }

    /// 基於提供的工具定義檢測是否包含 XML 工具調用
    pub fn contains_xml_tool_calls_with_tools(&self, tools: &[ChatTool]) -> bool {
        // 首先檢查通用格式
        if self.contains_xml_tool_calls() {
            return true;
        }

        // 檢查特定工具的標籤
        for tool in tools {
            let tool_tag = format!("<{}>", tool.function.name);
            if self.content.contains(&tool_tag) {
                return true;
            }
        }

        false
    }

    /// 從消息中提取 XML 工具調用
    pub fn extract_xml_tool_calls(&self) -> Vec<ChatToolCall> {
        XmlToolCallParser::parse_xml_tool_calls(&self.content)
    }

    /// 基於提供的工具定義從消息中提取 XML 工具調用
    pub fn extract_xml_tool_calls_with_tools(&self, tools: &[ChatTool]) -> Vec<ChatToolCall> {
        XmlToolCallParser::parse_xml_tool_calls_with_tools(&self.content, tools)
    }
}
