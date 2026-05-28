use serde_json::Value;

/// Extract text content from various content formats
pub fn extract_text_from_content(content: &Value) -> String {
    match content {
        Value::String(s) => s.clone(),
        Value::Array(arr) => {
            let mut parts = Vec::new();
            for item in arr {
                if let Some(text) = item.get("text").and_then(|t| t.as_str()) {
                    parts.push(text);
                }
            }
            parts.join("\n")
        }
        Value::Object(obj) => {
            if let Some(text) = obj.get("text").and_then(|t| t.as_str()) {
                text.to_string()
            } else {
                serde_json::to_string(obj).unwrap_or_default()
            }
        }
        _ => "".to_string(),
    }
}

/// Get the type of a content block
pub fn get_block_type(content: &Value) -> Option<String> {
    content.get("type").and_then(|t| t.as_str()).map(|s| s.to_string())
}

/// Get an attribute from a content block
pub fn get_block_attr(content: &Value, attr: &str) -> Option<Value> {
    content.get(attr).cloned()
}

/// Set a value if it's not None
pub fn set_if_not_none<T>(map: &mut serde_json::Map<String, Value>, key: &str, value: Option<T>)
where
    T: Into<Value>,
{
    if let Some(val) = value {
        map.insert(key.to_string(), val.into());
    }
}

/// Append request ID to an error message
pub fn append_request_id(error: &str, request_id: &str) -> String {
    format!("{} [request_id: {}]", error, request_id)
}

/// Extract text from a Message's ContentOrBlocks
pub fn extract_text_from_message_content(
    content: &crate::models::ContentOrBlocks,
) -> String {
    match content {
        crate::models::ContentOrBlocks::String(s) => s.clone(),
        crate::models::ContentOrBlocks::Blocks(blocks) => {
            let mut parts = Vec::new();
            for block in blocks {
                match block {
                    crate::models::ContentBlock::Text(t) => parts.push(t.text.clone()),
                    _ => {}
                }
            }
            parts.join("\n")
        }
    }
}

/// Extract text from SystemContentOrString
pub fn extract_text_from_system_content(
    content: &crate::models::SystemContentOrString,
) -> String {
    match content {
        crate::models::SystemContentOrString::String(s) => s.clone(),
        crate::models::SystemContentOrString::System(sys) => sys.text.clone(),
    }
}

