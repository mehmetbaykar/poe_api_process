use serde_json::Value;

/// Safely truncate a string by byte length to a safe Unicode boundary
pub(crate) fn truncate_str_by_bytes(s: &str, max: usize) -> (String, bool) {
    if s.len() <= max {
        return (s.to_string(), false);
    }
    
    let mut end = max;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    
    let truncated = format!("{}… [truncated {} bytes]", &s[..end], s.len() - end);
    (truncated, true)
}

/// Redact sensitive header values like Authorization and Cookie
pub(crate) fn redact_header(name: &str, value: &str) -> String {
    match name.to_ascii_lowercase().as_str() {
        "authorization" | "cookie" => "<redacted>".to_string(),
        _ => value.to_string(),
    }
}

/// Create a JSON representation of ChatRequest with message content truncated to 64KB
pub(crate) fn loggable_request_json(request: &serde_json::Value, max_message_bytes: usize) -> Value {
    let mut loggable = request.clone();
    
    if let Some(query_array) = loggable.get_mut("query").and_then(Value::as_array_mut) {
        for item in query_array {
            if let Some(content_str) = item.get("content").and_then(Value::as_str) {
                let (truncated, was_truncated) = truncate_str_by_bytes(content_str, max_message_bytes);
                if was_truncated {
                    if let Some(content_slot) = item.get_mut("content") {
                        *content_slot = Value::String(truncated);
                    }
                }
            }
        }
    }
    
    loggable
}

/// Truncate text fields in JSON values while preserving structure
pub(crate) fn truncate_text_fields(value: &Value, max_bytes: usize) -> Value {
    match value {
        Value::String(s) => {
            let (truncated, was_truncated) = truncate_str_by_bytes(s, max_bytes);
            Value::String(if was_truncated { truncated } else { s.clone() })
        }
        Value::Array(arr) => {
            Value::Array(arr.iter().map(|v| truncate_text_fields(v, max_bytes)).collect())
        }
        Value::Object(obj) => {
            Value::Object(obj.iter().map(|(k, v)| {
                (k.clone(), truncate_text_fields(v, max_bytes))
            }).collect())
        }
        _ => value.clone(),
    }
}
