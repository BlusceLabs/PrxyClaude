use tracing::{debug, info, warn};

/// Parse a CLI event and return a structured result.
pub fn parse_cli_event(event: &serde_json::Value, log_raw_cli: bool) -> Vec<serde_json::Value> {
    let etype = match event.get("type").and_then(|v| v.as_str()) {
        Some(t) => t,
        None => return Vec::new(),
    };

    // Ignore system events
    if etype == "system" {
        return Vec::new();
    }

    let mut results = Vec::new();

    // 1. Handle full messages (assistant/user or result)
    let mut msg_obj = None;
    if etype == "assistant" || etype == "user" {
        msg_obj = event.get("message");
    } else if etype == "result" {
        if let Some(res) = event.get("result") {
            if let Some(obj) = res.as_object() {
                msg_obj = obj.get("message");
                if msg_obj.is_none() {
                    if let Some(content) = obj.get("content") {
                        if content.is_array() {
                            msg_obj = Some(content);
                        }
                    }
                }
            }
        }
        if msg_obj.is_none() {
            msg_obj = event.get("message");
        }
        if msg_obj.is_none() {
            if let Some(content) = event.get("content") {
                if content.is_array() {
                    msg_obj = Some(content);
                }
            }
        }
    }

    if let Some(msg) = msg_obj {
        if let Some(content) = msg.get("content").and_then(|v| v.as_array()) {
            for c in content {
                if let Some(obj) = c.as_object() {
                    let ctype = obj.get("type").and_then(|v| v.as_str()).unwrap_or("");
                    match ctype {
                        "text" => {
                            let text = obj.get("text").and_then(|v| v.as_str()).unwrap_or("");
                            results.push(serde_json::json!({
                                "type": "text_chunk",
                                "text": text,
                            }));
                        }
                        "thinking" => {
                            let text = obj.get("thinking").and_then(|v| v.as_str()).unwrap_or("");
                            results.push(serde_json::json!({
                                "type": "thinking_chunk",
                                "text": text,
                            }));
                        }
                        "tool_use" => {
                            let id = obj.get("id").and_then(|v| v.as_str()).unwrap_or("").trim();
                            let name = obj.get("name").and_then(|v| v.as_str()).unwrap_or("");
                            let input = obj.get("input");
                            results.push(serde_json::json!({
                                "type": "tool_use",
                                "id": id,
                                "name": name,
                                "input": input,
                            }));
                        }
                        "tool_result" => {
                            let tool_use_id = obj.get("tool_use_id").and_then(|v| v.as_str()).unwrap_or("").trim();
                            let content_val = obj.get("content");
                            let is_error = obj.get("is_error").and_then(|v| v.as_bool()).unwrap_or(false);
                            results.push(serde_json::json!({
                                "type": "tool_result",
                                "tool_use_id": tool_use_id,
                                "content": content_val,
                                "is_error": is_error,
                            }));
                        }
                        _ => {}
                    }
                }
            }
            if !results.is_empty() {
                return results;
            }
        }
    }

    // 2. Handle streaming deltas
    if etype == "content_block_delta" {
        if let Some(delta) = event.get("delta") {
            if let Some(obj) = delta.as_object() {
                let dtype = obj.get("type").and_then(|v| v.as_str()).unwrap_or("");
                let index = event.get("index").and_then(|v| v.as_i64()).unwrap_or(-1);
                match dtype {
                    "text_delta" => {
                        let text = obj.get("text").and_then(|v| v.as_str()).unwrap_or("");
                        return vec![serde_json::json!({
                            "type": "text_delta",
                            "index": index,
                            "text": text,
                        })];
                    }
                    "thinking_delta" => {
                        let text = obj.get("thinking").and_then(|v| v.as_str()).unwrap_or("");
                        return vec![serde_json::json!({
                            "type": "thinking_delta",
                            "index": index,
                            "text": text,
                        })];
                    }
                    "input_json_delta" => {
                        let partial_json = obj.get("partial_json").and_then(|v| v.as_str()).unwrap_or("");
                        return vec![serde_json::json!({
                            "type": "tool_use_delta",
                            "index": index,
                            "partial_json": partial_json,
                        })];
                    }
                    _ => {}
                }
            }
        }
    }

    // 3. Handle tool usage start
    if etype == "content_block_start" {
        if let Some(block) = event.get("content_block") {
            if let Some(obj) = block.as_object() {
                let btype = obj.get("type").and_then(|v| v.as_str()).unwrap_or("");
                let index = event.get("index").and_then(|v| v.as_i64()).unwrap_or(-1);
                match btype {
                    "thinking" => {
                        return vec![serde_json::json!({"type": "thinking_start", "index": index})];
                    }
                    "text" => {
                        return vec![serde_json::json!({"type": "text_start", "index": index})];
                    }
                    "tool_use" => {
                        let id = obj.get("id").and_then(|v| v.as_str()).unwrap_or("").trim();
                        let name = obj.get("name").and_then(|v| v.as_str()).unwrap_or("");
                        let input = obj.get("input");
                        return vec![serde_json::json!({
                            "type": "tool_use_start",
                            "index": index,
                            "id": id,
                            "name": name,
                            "input": input,
                        })];
                    }
                    _ => {}
                }
            }
        }
    }

    // 3.5 Handle block stop
    if etype == "content_block_stop" {
        let index = event.get("index").and_then(|v| v.as_i64()).unwrap_or(-1);
        return vec![serde_json::json!({"type": "block_stop", "index": index})];
    }

    // 4. Handle errors and exit
    if etype == "error" {
        let msg = event
            .get("error")
            .and_then(|e| {
                if let Some(obj) = e.as_object() {
                    obj.get("message").and_then(|v| v.as_str()).map(String::from)
                } else {
                    Some(e.to_string())
                }
            })
            .unwrap_or_else(|| "Unknown error".to_string());

        if log_raw_cli {
            info!("CLI_PARSER: Parsed error event: {msg}");
        } else {
            info!("CLI_PARSER: Parsed error event: message_chars={}", msg.len());
        }

        return vec![
            serde_json::json!({"type": "error", "message": msg}),
            serde_json::json!({"type": "complete", "status": "failed"}),
        ];
    }

    if etype == "exit" {
        let code = event.get("code").and_then(|v| v.as_i64()).unwrap_or(0);
        let stderr = event.get("stderr").and_then(|v| v.as_str());

        if code == 0 {
            debug!("CLI_PARSER: Successful exit (code={code})");
            return vec![serde_json::json!({"type": "complete", "status": "success"})];
        } else {
            let error_msg = stderr
                .unwrap_or(&format!("Process exited with code {code}"))
                .to_string();
            if log_raw_cli {
                warn!("CLI_PARSER: Error exit (code={code}): {error_msg}");
            } else {
                warn!(
                    "CLI_PARSER: Error exit (code={code}): message_chars={}",
                    error_msg.len()
                );
            }
            return vec![
                serde_json::json!({"type": "error", "message": error_msg}),
                serde_json::json!({"type": "complete", "status": "failed"}),
            ];
        }
    }

    if !etype.is_empty() {
        debug!("CLI_PARSER: Unrecognized event type: {etype}");
    }
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_system_event() {
        let event = serde_json::json!({"type": "system"});
        assert!(parse_cli_event(&event, false).is_empty());
    }

    #[test]
    fn test_parse_text_delta() {
        let event = serde_json::json!({
            "type": "content_block_delta",
            "index": 0,
            "delta": {"type": "text_delta", "text": "hello"}
        });
        let results = parse_cli_event(&event, false);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0]["type"], "text_delta");
    }

    #[test]
    fn test_parse_thinking_start() {
        let event = serde_json::json!({
            "type": "content_block_start",
            "index": 0,
            "content_block": {"type": "thinking"}
        });
        let results = parse_cli_event(&event, false);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0]["type"], "thinking_start");
    }

    #[test]
    fn test_parse_error() {
        let event = serde_json::json!({
            "type": "error",
            "error": {"message": "something went wrong"}
        });
        let results = parse_cli_event(&event, false);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0]["type"], "error");
    }

    #[test]
    fn test_parse_exit_success() {
        let event = serde_json::json!({"type": "exit", "code": 0});
        let results = parse_cli_event(&event, false);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0]["status"], "success");
    }

    #[test]
    fn test_parse_exit_failure() {
        let event = serde_json::json!({"type": "exit", "code": 1, "stderr": "error output"});
        let results = parse_cli_event(&event, false);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0]["type"], "error");
    }

    #[test]
    fn test_parse_tool_use_start() {
        let event = serde_json::json!({
            "type": "content_block_start",
            "index": 0,
            "content_block": {"type": "tool_use", "id": "t1", "name": "Bash"}
        });
        let results = parse_cli_event(&event, false);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0]["type"], "tool_use_start");
        assert_eq!(results[0]["name"], "Bash");
    }
}
