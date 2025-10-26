use crate::types::{
    ChatMessage, ChatRequest, ChatTool, ChatToolCall, ChatToolResult, FunctionCall,
};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};

// Global tool call ID counter, ensuring each tool call has a unique ID
static GLOBAL_CALL_ID: AtomicU64 = AtomicU64::new(1);

// Generate the next unique tool call ID
fn get_next_call_id() -> u64 {
    GLOBAL_CALL_ID.fetch_add(1, Ordering::SeqCst)
}

#[cfg(feature = "trace")]
fn safe_string_truncate(s: &str, max_bytes: usize) -> &str {
    if s.len() <= max_bytes {
        return s;
    }

    // From the max_bytes position, search backwards to find a valid character boundary
    let mut end = max_bytes;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }

    &s[..end]
}

// XML tool format related structures
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

// XML tool conversion trait
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

            if let Some(properties) = parameters
                .properties
                .as_ref()
                .and_then(|value| value.as_object())
            {
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

        // Check if content is in error format
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

// XML escape function
fn escape_xml(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

// Add XML tool attachment functionality to ChatMessage (internal use only)
impl ChatMessage {
    /// Append XML format tool definitions to the end of message content (internal use)
    pub(crate) fn append_xml_tools(&mut self, tools: &[ChatTool]) {
        if !tools.is_empty() {
            let tools_vec = tools.to_vec();
            let xml_tools = tools_vec.to_xml();
            self.content.push_str(&xml_tools);
        }
    }

    /// Append XML format tool results to the end of message content (internal use)
    pub(crate) fn append_xml_tool_results(&mut self, tool_results: &[ChatToolResult]) {
        if !tool_results.is_empty() {
            let results_vec = tool_results.to_vec();
            let xml_results = results_vec.to_xml();
            self.content.push_str(&xml_results);
        }
    }
}

// Add XML tool processing functionality to ChatRequest (internal use only)
impl ChatRequest {
    /// Convert tools to XML format and append to the last user message (internal use)
    pub(crate) fn append_tools_as_xml(&mut self) {
        if let Some(ref tools) = self.tools {
            if !tools.is_empty() {
                // Find the last user message
                for message in self.query.iter_mut().rev() {
                    if message.role == "user" {
                        // Add complete tool usage prompt
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

    /// Append tool results in XML format to the last user message (internal use)
    pub(crate) fn append_tool_results_as_xml(&mut self) {
        if let Some(ref tool_results) = self.tool_results {
            if !tool_results.is_empty() {
                // Find the last user message
                for message in self.query.iter_mut().rev() {
                    if message.role == "user" {
                        // Add tool result analysis prompt
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

// XML tool call parsing functionality
pub struct XmlToolCallParser;

impl XmlToolCallParser {
    /// Parse XML tool calls from text
    pub fn parse_xml_tool_calls(text: &str) -> Vec<ChatToolCall> {
        let mut tool_calls = Vec::new();

        #[cfg(feature = "trace")]
        {
            use tracing::debug;
            debug!(
                "Starting XML tool call parsing, text length: {}",
                text.len()
            );
            debug!("Text preview: {}", text);
        }

        // First, find <tool_call> wrapped tool calls
        let mut current_pos = 0;
        while let Some(call_start) = text[current_pos..].find("<tool_call>") {
            let actual_start = current_pos + call_start;

            if let Some(call_end) = text[actual_start..].find("</tool_call>") {
                let actual_end = actual_start + call_end + "</tool_call>".len();
                let call_content = &text[actual_start..actual_end];

                let current_call_id = get_next_call_id();
                #[cfg(feature = "trace")]
                {
                    use tracing::debug;
                    debug!(
                        "Found complete tool call #{}, start position: {}, end position: {}",
                        current_call_id, actual_start, actual_end
                    );
                    debug!("Tool call content: {}", call_content);
                }

                if let Some(tool_call) = Self::parse_single_tool_call(call_content, current_call_id)
                {
                    #[cfg(feature = "trace")]
                    {
                        use tracing::debug;
                        debug!(
                            "Successfully parsed tool call #{}: {}",
                            current_call_id, tool_call.function.name
                        );
                    }
                    tool_calls.push(tool_call);
                } else {
                    #[cfg(feature = "trace")]
                    {
                        use tracing::debug;
                        debug!("Could not parse tool call #{}", current_call_id);
                    }
                }

                current_pos = actual_end;
            } else {
                #[cfg(feature = "trace")]
                {
                    use tracing::debug;
                    debug!("Found <tool_call> but no corresponding </tool_call>, stopping parsing");
                }
                break;
            }
        }

        // If no <tool_call> wrapped calls are found, directly search for <invoke> tags
        if tool_calls.is_empty() {
            current_pos = 0;
            while let Some(invoke_start) = text[current_pos..].find("<invoke") {
                let actual_start = current_pos + invoke_start;

                if let Some(invoke_end) = text[actual_start..].find("</invoke>") {
                    let actual_end = actual_start + invoke_end + "</invoke>".len();
                    let invoke_content = &text[actual_start..actual_end];

                    let current_call_id = get_next_call_id();
                    #[cfg(feature = "trace")]
                    {
                        use tracing::debug;
                        debug!(
                            "Found direct invoke call #{}, start position: {}, end position: {}",
                            current_call_id, actual_start, actual_end
                        );
                        debug!("invoke call content: {}", invoke_content);
                    }

                    if let Some(tool_call) =
                        Self::parse_single_tool_call(invoke_content, current_call_id)
                    {
                        #[cfg(feature = "trace")]
                        {
                            use tracing::debug;
                            debug!(
                                "Successfully parsed direct invoke call #{}: {}",
                                current_call_id, tool_call.function.name
                            );
                        }
                        tool_calls.push(tool_call);
                    } else {
                        #[cfg(feature = "trace")]
                        {
                            use tracing::debug;
                            debug!("Could not parse direct invoke call #{}", current_call_id);
                        }
                    }

                    current_pos = actual_end;
                } else {
                    #[cfg(feature = "trace")]
                    {
                        use tracing::debug;
                        debug!("Found <invoke but no corresponding </invoke>, stopping parsing");
                    }
                    break;
                }
            }
        }

        #[cfg(feature = "trace")]
        {
            use tracing::debug;
            debug!(
                "XML tool call parsing complete, found {} tool calls",
                tool_calls.len()
            );
        }

        tool_calls
    }

    /// Parse XML tool calls based on provided tool definitions from text
    pub fn parse_xml_tool_calls_with_tools(text: &str, tools: &[ChatTool]) -> Vec<ChatToolCall> {
        let mut tool_calls = Vec::new();

        // First, try standard format
        tool_calls.extend(Self::parse_xml_tool_calls(text));

        // If standard tool call format is not found, try tool-specific parsing
        if tool_calls.is_empty() {
            tool_calls.extend(Self::parse_tool_specific_xml_format(text, tools));
        } else {
            // If standard tool call format is found, but still want to try tool-specific format
            let additional_calls = Self::parse_tool_specific_xml_format(text, tools);

            // Only add tool calls that were not found in the standard format
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

    /// Parse a single tool call
    fn parse_single_tool_call(xml_content: &str, call_id: u64) -> Option<ChatToolCall> {
        #[cfg(feature = "trace")]
        {
            use tracing::debug;
            debug!(
                "Attempting to parse single tool call, content length: {}",
                xml_content.len()
            );
            debug!(
                "XML content preview: {}",
                safe_string_truncate(xml_content, 200)
            );
        }

        // First, try to parse <invoke name="tool_name"> format
        if let Some(function_name) = Self::extract_invoke_name(xml_content) {
            let arguments = Self::extract_parameters_as_json(xml_content);

            #[cfg(feature = "trace")]
            {
                use tracing::debug;
                debug!(
                    "Successfully parsed invoke format, tool name: {}, arguments: {}",
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

        // If invoke tag is not found, try old format
        if let Some(function_name) = Self::extract_xml_value(xml_content, "name") {
            let arguments = Self::extract_xml_value(xml_content, "arguments")
                .unwrap_or_else(|| Self::extract_parameters_as_json(xml_content));

            #[cfg(feature = "trace")]
            {
                use tracing::debug;
                debug!(
                    "Successfully parsed old format, tool name: {}, arguments: {}",
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

        // Try direct tool name tag format
        if let Some((function_name, tool_content)) =
            Self::extract_direct_tool_name_and_content(xml_content)
        {
            let arguments = Self::extract_parameters_as_json(&tool_content);

            #[cfg(feature = "trace")]
            {
                use tracing::debug;
                debug!(
                    "Successfully parsed direct tool name format, tool name: {}, arguments: {}",
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
            debug!("Could not parse tool call, no valid tool name found");
        }

        None
    }

    /// Extract direct tool name tag format and return tool name and content
    fn extract_direct_tool_name_and_content(xml_content: &str) -> Option<(String, String)> {
        // Skip <tool_call> tag, find the internal tool tag
        let start_marker = "<tool_call>";
        let end_marker = "</tool_call>";

        if let Some(start_pos) = xml_content.find(start_marker) {
            let content_start = start_pos + start_marker.len();
            if let Some(end_pos) = xml_content.find(end_marker) {
                let inner_content = &xml_content[content_start..end_pos];

                // Find the first non-whitespace character after the < tag
                let trimmed = inner_content.trim();
                if trimmed.starts_with('<') {
                    // Find the first >
                    if let Some(tag_end) = trimmed.find('>') {
                        let tag_content = &trimmed[1..tag_end];

                        // Exclude special tags
                        if !tag_content.starts_with('/')
                            && !tag_content.starts_with('!')
                            && !tag_content.contains("invoke")
                            && !tag_content.contains("parameter")
                            && !tag_content.contains(' ')
                        {
                            // Find the corresponding end tag
                            let end_tag = format!("</{}>", tag_content);
                            if let Some(tool_end_pos) = trimmed.find(&end_tag) {
                                let tool_content = &trimmed[tag_end + 1..tool_end_pos];

                                #[cfg(feature = "trace")]
                                {
                                    use tracing::debug;
                                    debug!("Extracted direct tool name: {}", tag_content);
                                    debug!("Tool content: {}", tool_content);
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

    /// Parse XML format based on provided tool definitions
    fn parse_tool_specific_xml_format(text: &str, tools: &[ChatTool]) -> Vec<ChatToolCall> {
        let mut tool_calls = Vec::new();

        for tool in tools {
            // Parse all call instances of this tool
            let mut current_pos = 0;
            while let Some(tool_call) = Self::parse_tool_tag_from_position(
                text,
                &tool.function.name,
                get_next_call_id(),
                current_pos,
            ) {
                tool_calls.push(tool_call);

                // Update search position to avoid re-parsing the same tool call
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

    /// Parse specific tool tag from a given position
    fn parse_tool_tag_from_position(
        text: &str,
        tool_name: &str,
        call_id: u64,
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

    /// Extract value from a specified tag
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

    /// Extract tool name from <invoke name="tool_name"> format
    fn extract_invoke_name(xml: &str) -> Option<String> {
        // Find <invoke name="..."> pattern
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

    /// Convert XML parameters to JSON format
    fn extract_parameters_as_json(xml_content: &str) -> String {
        let mut params = HashMap::new();

        // First, try to parse <parameter name="key">value</parameter> format
        let mut current_pos = 0;
        while let Some(param_start) = xml_content[current_pos..].find("<parameter") {
            let actual_start = current_pos + param_start;

            // Extract parameter name
            if let Some(name_start) = xml_content[actual_start..].find("name=\"") {
                let name_content_start = actual_start + name_start + 6; // 6 = len("name=\"")
                if let Some(name_end) = xml_content[name_content_start..].find('"') {
                    let param_name =
                        xml_content[name_content_start..name_content_start + name_end].to_string();

                    // Find parameter value
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
                                // Decode XML entities
                                let decoded_value = Self::decode_xml_entities(param_value);
                                params.insert(param_name, decoded_value);
                            }
                        }
                    }
                }
            }

            current_pos = actual_start + 1;
        }

        // If parameter tag is not found, try to parse all tags as parameters
        if params.is_empty() {
            let mut current_pos = 0;
            while current_pos < xml_content.len() {
                if let Some(tag_start) = xml_content[current_pos..].find('<') {
                    let actual_start = current_pos + tag_start;

                    // Skip end tags, comments, and special tags
                    if xml_content[actual_start..].starts_with("</")
                        || xml_content[actual_start..].starts_with("<!--")
                        || xml_content[actual_start..].starts_with("<invoke")
                        || xml_content[actual_start..].starts_with("<parameter")
                        || xml_content[actual_start..].starts_with("<tool_call")
                    {
                        current_pos = actual_start + 1;
                        continue;
                    }

                    // Find tag end
                    if let Some(tag_end) = xml_content[actual_start + 1..].find('>') {
                        let tag_name = &xml_content[actual_start + 1..actual_start + 1 + tag_end];

                        // Skip self-closing tags and tags with attributes
                        if tag_name.contains(' ') || tag_name.ends_with('/') {
                            current_pos = actual_start + 1 + tag_end + 1;
                            continue;
                        }

                        let content_start = actual_start + 1 + tag_end + 1;

                        // Find the corresponding end tag
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

        // Convert to JSON
        if params.is_empty() {
            "{}".to_string()
        } else {
            serde_json::to_string(&params).unwrap_or_else(|_| "{}".to_string())
        }
    }

    /// Decode XML entities
    fn decode_xml_entities(text: &str) -> String {
        text.replace("&lt;", "<")
            .replace("&gt;", ">")
            .replace("&amp;", "&")
            .replace("&quot;", "\"")
            .replace("&apos;", "'")
    }
}

// Add XML tool call detection functionality to ChatMessage
impl ChatMessage {
    /// Detect if message contains XML tool calls (generic format)
    pub fn contains_xml_tool_calls(&self) -> bool {
        // Check standard <tool_call> format - must have complete start and end tags
        if self.content.contains("<tool_call>") && self.content.contains("</tool_call>") {
            return true;
        }

        // Check <invoke> format - must have complete start and end tags
        if self.content.contains("<invoke") && self.content.contains("</invoke>") {
            return true;
        }

        false
    }

    /// Detect if XML tool calls are contained based on provided tool definitions
    pub fn contains_xml_tool_calls_with_tools(&self, tools: &[ChatTool]) -> bool {
        // First, check generic format
        if self.contains_xml_tool_calls() {
            return true;
        }

        // Check specific tool tags
        for tool in tools {
            let tool_tag = format!("<{}>", tool.function.name);
            if self.content.contains(&tool_tag) {
                return true;
            }
        }

        false
    }

    /// Extract XML tool calls from message
    pub fn extract_xml_tool_calls(&self) -> Vec<ChatToolCall> {
        XmlToolCallParser::parse_xml_tool_calls(&self.content)
    }

    /// Extract XML tool calls from message based on provided tool definitions
    pub fn extract_xml_tool_calls_with_tools(&self, tools: &[ChatTool]) -> Vec<ChatToolCall> {
        XmlToolCallParser::parse_xml_tool_calls_with_tools(&self.content, tools)
    }
}
