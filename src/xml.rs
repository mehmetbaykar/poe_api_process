use crate::types::{
    ChatMessage, ChatRequest, ChatTool, ChatToolCall, ChatToolResult, FunctionCall,
};
use std::collections::HashMap;

#[cfg(feature = "trace")]
fn safe_string_truncate(s: &str, max_bytes: usize) -> &str {
    if s.len() <= max_bytes {
        return s;
    }

    // 從 max_bytes 位置向前查找，直到找到有效的字符邊界
    let mut end = max_bytes;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }

    &s[..end]
}

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

impl ToXml for ChatToolResult {
    fn to_xml(&self) -> String {
        let mut xml = String::new();
        xml.push_str(&format!(
            "  <result tool_call_id=\"{}\">",
            escape_xml(&self.tool_call_id)
        ));

        // 檢查內容是否為錯誤格式
        if self.content.trim().starts_with("ERROR:") || self.content.trim().starts_with("Error:") {
            xml.push_str("\n    <error>");
            xml.push_str(&escape_xml(&self.content));
            xml.push_str("</error>");
        } else {
            xml.push_str("\n    <output>");
            xml.push_str(&escape_xml(&self.content));
            xml.push_str("</output>");
        }

        xml.push_str("\n  </result>");
        xml
    }
}

impl ToXml for Vec<ChatToolResult> {
    fn to_xml(&self) -> String {
        if self.is_empty() {
            return String::new();
        }

        let mut xml = String::from("\n\n<tool_results>");
        for result in self {
            xml.push('\n');
            xml.push_str(&result.to_xml());
        }
        xml.push_str("\n</tool_results>");
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

    /// 將 XML 格式的工具結果附加到消息內容末尾（內部使用）
    pub(crate) fn append_xml_tool_results(&mut self, tool_results: &[ChatToolResult]) {
        if !tool_results.is_empty() {
            let results_vec = tool_results.to_vec();
            let xml_results = results_vec.to_xml();
            self.content.push_str(&xml_results);
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
                        // 添加完整的工具使用提示詞
                        let tool_usage_prompt = r#"

You are a powerful AI assistant. Your core mission is to accurately and efficiently answer user questions and execute tasks.

To achieve this, you have been given a set of tools. When you determine that using a tool can fetch real-time information, perform a specific action, or provide a more precise answer than your built-in knowledge allows, you MUST proactively use these tools. Do not rely solely on your training data.

Tool Calling Rules:

1.  Be Proactive: Actively look for opportunities to use your tools. If you think a tool might help the user, use it.

2.  Strict Formatting: All tool calls must strictly adhere to the following XML format. This is not a suggestion; it is a mandatory requirement.

XML Calling Format Example:

When you need to call a tool, your response MUST ONLY contain XML blocks with the following structure.

<tool_call>

  <invoke name="tool_name">

    <parameter name="parameter_1_name">value_for_parameter_1</parameter>

    <parameter name="parameter_2_name">value_for_parameter_2</parameter>

    <!-- Add more parameters as needed -->

  </invoke>

</tool_call>

<!-- If you need to call multiple tools at once, you can place multiple <tool_call> blocks sequentially like this -->

<tool_call>

  <invoke name="another_tool_name">

    <parameter name="parameter_A">value_A</parameter>

  </invoke>

</tool_call>

Explanation:

- <tool_call>: The outermost wrapper for each individual tool call.

- <invoke name="...">: The name attribute must be the exact name of the tool you are calling.

- <parameter name="...">: The name attribute is the name of the parameter the tool requires, and the content between the tags is its value. All parameter values must be properly XML-escaped (e.g., & must be written as &amp;).

Now, begin your work based on the user's next prompt. Remember, you are a problem-solver, and your tools are your most powerful weapons.
"#;
                        message.content.push_str(tool_usage_prompt);
                        message.append_xml_tools(tools);
                        break;
                    }
                }
            }
        }
    }

    /// 將工具結果以 XML 格式附加到最後一條用戶消息中（內部使用）
    pub(crate) fn append_tool_results_as_xml(&mut self) {
        if let Some(ref tool_results) = self.tool_results {
            if !tool_results.is_empty() {
                // 找到最後一條用戶消息
                for message in self.query.iter_mut().rev() {
                    if message.role == "user" {
                        // 添加工具結果分析提示詞
                        let tool_results_prompt = r#"

You have previously requested one or more tool calls. The results are now available. Your new task is to analyze these results and formulate a final, comprehensive answer for the user in natural language.

The tool results are provided to you in the following XML format:

**Your Instructions:**

1.  **Analyze the Results**: Carefully examine the content within the `<output>` or `<error>` tags for each result.
2.  **Synthesize, Don't Recite**: Do not just repeat the raw tool output (like raw JSON). You **must interpret** the data, synthesize information if there are multiple results, and present it to the user in a clear, conversational, and helpful way.
3.  **Formulate the Final Answer**: Your response should be the complete and final answer to the user's original query. Do not output any more `<tool_call>` blocks unless the results explicitly indicate a necessary follow-up action.
4.  **Handle Errors Gracefully**: If a tool returned an error, politely inform the user that you were unable to retrieve that specific piece of information and, if appropriate, briefly explain the issue (e.g., "I couldn't find information for that city.").
"#;
                        message.content.push_str(tool_results_prompt);
                        message.append_xml_tool_results(tool_results);
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

        #[cfg(feature = "trace")]
        {
            use tracing::debug;
            debug!("開始解析 XML 工具調用，文本長度: {}", text.len());
            debug!("文本內容預覽: {}", safe_string_truncate(text, 300));
        }

        // 查找所有的工具調用標籤
        let mut current_pos = 0;
        let mut call_id = 1;

        while let Some(call_start) = text[current_pos..].find("<tool_call>") {
            let actual_start = current_pos + call_start;

            if let Some(call_end) = text[actual_start..].find("</tool_call>") {
                let actual_end = actual_start + call_end + "</tool_call>".len();
                let call_content = &text[actual_start..actual_end];

                #[cfg(feature = "trace")]
                {
                    use tracing::debug;
                    debug!(
                        "找到完整的工具調用 #{}, 開始位置: {}, 結束位置: {}",
                        call_id, actual_start, actual_end
                    );
                    debug!("工具調用內容: {}", call_content);
                }

                if let Some(tool_call) = Self::parse_single_tool_call(call_content, call_id) {
                    #[cfg(feature = "trace")]
                    {
                        use tracing::debug;
                        debug!("成功解析工具調用 #{}: {}", call_id, tool_call.function.name);
                    }
                    tool_calls.push(tool_call);
                    call_id += 1;
                } else {
                    #[cfg(feature = "trace")]
                    {
                        use tracing::debug;
                        debug!("無法解析工具調用 #{}", call_id);
                    }
                }

                current_pos = actual_end;
            } else {
                #[cfg(feature = "trace")]
                {
                    use tracing::debug;
                    debug!("找到 <tool_call> 但沒有找到對應的 </tool_call>，停止解析");
                }
                break;
            }
        }

        #[cfg(feature = "trace")]
        {
            use tracing::debug;
            debug!(
                "XML 工具調用解析完成，共找到 {} 個工具調用",
                tool_calls.len()
            );
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
            tool_calls.extend(Self::parse_tool_specific_xml_format(text, tools, 1));
        } else {
            // 如果已經找到了標準格式的工具調用，但還想嘗試工具特定格式
            // 使用下一個可用的 ID 來避免重複
            let next_id = tool_calls.len() as u32 + 1;
            let additional_calls = Self::parse_tool_specific_xml_format(text, tools, next_id);

            // 只添加那些在標準格式中沒有找到的工具調用
            for additional_call in additional_calls {
                let already_exists = tool_calls.iter().any(|existing| {
                    existing.function.name == additional_call.function.name
                        && existing.function.arguments == additional_call.function.arguments
                });

                if !already_exists {
                    tool_calls.push(additional_call);
                }
            }
        }

        tool_calls
    }

    /// 解析單個工具調用
    fn parse_single_tool_call(xml_content: &str, call_id: u32) -> Option<ChatToolCall> {
        #[cfg(feature = "trace")]
        {
            use tracing::debug;
            debug!("嘗試解析單個工具調用，內容長度: {}", xml_content.len());
            debug!("XML 內容預覽: {}", safe_string_truncate(xml_content, 200));
        }

        // 首先嘗試解析 <invoke name="tool_name"> 格式
        if let Some(function_name) = Self::extract_invoke_name(xml_content) {
            let arguments = Self::extract_parameters_as_json(xml_content);

            #[cfg(feature = "trace")]
            {
                use tracing::debug;
                debug!(
                    "成功解析 invoke 格式，工具名: {}, 參數: {}",
                    function_name, arguments
                );
            }

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

            #[cfg(feature = "trace")]
            {
                use tracing::debug;
                debug!(
                    "成功解析舊格式，工具名: {}, 參數: {}",
                    function_name, arguments
                );
            }

            return Some(ChatToolCall {
                id: format!("call_{}", call_id),
                r#type: "function".to_string(),
                function: FunctionCall {
                    name: function_name,
                    arguments,
                },
            });
        }

        // 嘗試直接工具名稱標籤格式
        if let Some((function_name, tool_content)) =
            Self::extract_direct_tool_name_and_content(xml_content)
        {
            let arguments = Self::extract_parameters_as_json(&tool_content);

            #[cfg(feature = "trace")]
            {
                use tracing::debug;
                debug!(
                    "成功解析直接工具名稱格式，工具名: {}, 參數: {}",
                    function_name, arguments
                );
            }

            return Some(ChatToolCall {
                id: format!("call_{}", call_id),
                r#type: "function".to_string(),
                function: FunctionCall {
                    name: function_name,
                    arguments,
                },
            });
        }

        #[cfg(feature = "trace")]
        {
            use tracing::debug;
            debug!("無法解析工具調用，未找到有效的工具名稱");
        }

        None
    }

    /// 提取直接工具名稱標籤格式並返回工具名稱和內容
    fn extract_direct_tool_name_and_content(xml_content: &str) -> Option<(String, String)> {
        // 跳過 <tool_call> 標籤，查找內部的工具標籤
        let start_marker = "<tool_call>";
        let end_marker = "</tool_call>";

        if let Some(start_pos) = xml_content.find(start_marker) {
            let content_start = start_pos + start_marker.len();
            if let Some(end_pos) = xml_content.find(end_marker) {
                let inner_content = &xml_content[content_start..end_pos];

                // 查找第一個非空白字符後的 < 標籤
                let trimmed = inner_content.trim();
                if trimmed.starts_with('<') {
                    // 找到第一個 >
                    if let Some(tag_end) = trimmed.find('>') {
                        let tag_content = &trimmed[1..tag_end];

                        // 排除特殊標籤
                        if !tag_content.starts_with('/')
                            && !tag_content.starts_with('!')
                            && !tag_content.contains("invoke")
                            && !tag_content.contains("parameter")
                            && !tag_content.contains(' ')
                        {
                            // 找到對應的結束標籤
                            let end_tag = format!("</{}>", tag_content);
                            if let Some(tool_end_pos) = trimmed.find(&end_tag) {
                                let tool_content = &trimmed[tag_end + 1..tool_end_pos];

                                #[cfg(feature = "trace")]
                                {
                                    use tracing::debug;
                                    debug!("提取到直接工具名稱: {}", tag_content);
                                    debug!("工具內容: {}", tool_content);
                                }

                                return Some((tag_content.to_string(), tool_content.to_string()));
                            }
                        }
                    }
                }
            }
        }

        None
    }

    /// 基於提供的工具定義解析 XML 格式，使用指定的起始 ID
    fn parse_tool_specific_xml_format(
        text: &str,
        tools: &[ChatTool],
        start_id: u32,
    ) -> Vec<ChatToolCall> {
        let mut tool_calls = Vec::new();
        let mut call_id = start_id;

        for tool in tools {
            // 解析該工具的所有調用實例
            let mut current_pos = 0;
            while let Some(tool_call) =
                Self::parse_tool_tag_from_position(text, &tool.function.name, call_id, current_pos)
            {
                tool_calls.push(tool_call);
                call_id += 1;

                // 更新搜索位置，避免重複解析同一個工具調用
                if let Some(start_tag_pos) =
                    text[current_pos..].find(&format!("<{}>", tool.function.name))
                {
                    current_pos += start_tag_pos + format!("<{}>", tool.function.name).len();
                } else {
                    break;
                }
            }
        }

        tool_calls
    }

    /// 從指定位置開始解析特定工具標籤
    fn parse_tool_tag_from_position(
        text: &str,
        tool_name: &str,
        call_id: u32,
        start_from: usize,
    ) -> Option<ChatToolCall> {
        let start_tag = format!("<{}>", tool_name);
        let end_tag = format!("</{}>", tool_name);

        if let Some(start_pos) = text[start_from..].find(&start_tag) {
            let actual_start = start_from + start_pos;
            let content_start = actual_start + start_tag.len();
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

        // 如果沒有找到 parameter 標籤，嘗試解析所有標籤作為參數
        if params.is_empty() {
            let mut current_pos = 0;
            while current_pos < xml_content.len() {
                if let Some(tag_start) = xml_content[current_pos..].find('<') {
                    let actual_start = current_pos + tag_start;

                    // 跳過結束標籤、註釋和特殊標籤
                    if xml_content[actual_start..].starts_with("</")
                        || xml_content[actual_start..].starts_with("<!--")
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

                        // 跳過自閉合標籤和包含屬性的標籤
                        if tag_name.contains(' ') || tag_name.ends_with('/') {
                            current_pos = actual_start + 1 + tag_end + 1;
                            continue;
                        }

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
        // 檢測標準的 <tool_call> 格式 - 必須有完整的開始和結束標籤
        if self.content.contains("<tool_call>") && self.content.contains("</tool_call>") {
            return true;
        }

        // 檢測 <invoke> 格式 - 必須有完整的開始和結束標籤
        if self.content.contains("<invoke") && self.content.contains("</invoke>") {
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
