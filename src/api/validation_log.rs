use serde_json::Value;

pub fn summarize_request_validation_body(body: &Value) -> (Vec<Value>, Vec<String>) {
    let mut message_summary = Vec::new();
    let mut tool_names = Vec::new();

    if let Some(messages) = body.get("messages").and_then(|v| v.as_array()) {
        for msg in messages {
            let mut item = serde_json::Map::new();
            if let Some(role) = msg.get("role").and_then(|v| v.as_str()) {
                item.insert("role".to_string(), Value::String(role.to_string()));
            }
            if let Some(content) = msg.get("content") {
                let kind = match content {
                    Value::String(s) => {
                        item.insert(
                            "content_length".to_string(),
                            Value::Number(s.len().into()),
                        );
                        "str"
                    }
                    Value::Array(blocks) => {
                        let block_types: Vec<Value> = blocks
                            .iter()
                            .take(12)
                            .map(|b| {
                                b.get("type")
                                    .and_then(|v| v.as_str())
                                    .map(|s| Value::String(s.to_string()))
                                    .unwrap_or(Value::String("dict".to_string()))
                            })
                            .collect();
                        item.insert(
                            "block_types".to_string(),
                            Value::Array(block_types),
                        );
                        "list"
                    }
                    _ => "other",
                };
                item.insert(
                    "content_kind".to_string(),
                    Value::String(kind.to_string()),
                );
            }
            message_summary.push(Value::Object(item));
        }
    }

    if let Some(tools) = body.get("tools").and_then(|v| v.as_array()) {
        for tool in tools {
            if let Some(name) = tool.get("name").and_then(|v| v.as_str()) {
                tool_names.push(name.to_string());
            }
        }
    }

    (message_summary, tool_names)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_summarize_empty_body() {
        let body = serde_json::json!({});
        let (summaries, tools) = summarize_request_validation_body(&body);
        assert!(summaries.is_empty());
        assert!(tools.is_empty());
    }

    #[test]
    fn test_summarize_with_messages() {
        let body = serde_json::json!({
            "messages": [
                {"role": "user", "content": "hello"},
                {"role": "assistant", "content": [{"type": "text", "text": "hi"}]}
            ]
        });
        let (summaries, tools) = summarize_request_validation_body(&body);
        assert_eq!(summaries.len(), 2);
        assert_eq!(summaries[0]["role"].as_str(), Some("user"));
        assert_eq!(summaries[1]["content_kind"].as_str(), Some("list"));
        assert!(tools.is_empty());
    }

    #[test]
    fn test_summarize_with_tools() {
        let body = serde_json::json!({
            "tools": [
                {"name": "web_search", "description": "Search the web"},
                {"name": "calculator", "description": "Do math"}
            ]
        });
        let (_, tools) = summarize_request_validation_body(&body);
        assert_eq!(tools, vec!["web_search", "calculator"]);
    }
}
