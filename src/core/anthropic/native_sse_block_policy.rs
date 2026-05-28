use serde_json::Value;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone)]
struct UpstreamBlockState {
    #[allow(dead_code)]
    block_type: String,
    down_index: usize,
    open: bool,
    last_start_block: Option<Value>,
}

#[derive(Debug, Clone)]
pub struct NativeSseBlockPolicyState {
    pub next_index: usize,
    by_upstream: HashMap<usize, UpstreamBlockState>,
    pub dropped_indexes: HashSet<usize>,
    pending_suppressed_stops: HashSet<usize>,
    pub message_stopped: bool,
}

impl NativeSseBlockPolicyState {
    pub fn new() -> Self {
        Self {
            next_index: 0,
            by_upstream: HashMap::new(),
            dropped_indexes: HashSet::new(),
            pending_suppressed_stops: HashSet::new(),
            message_stopped: false,
        }
    }
}

pub fn format_native_sse_event(event_name: Option<&str>, data_text: &str) -> String {
    let mut lines = Vec::new();
    if let Some(name) = event_name {
        lines.push(format!("event: {}", name));
    }
    for line in data_text.lines() {
        lines.push(format!("data: {}", line));
    }
    lines.join("\n") + "\n\n"
}

pub fn parse_native_sse_event(event: &str) -> (Option<String>, String) {
    let mut event_name = None;
    let mut data_lines = Vec::new();
    for line in event.trim().lines() {
        if let Some(val) = line.strip_prefix("event:") {
            event_name = Some(val.trim().to_string());
        } else if let Some(val) = line.strip_prefix("data:") {
            data_lines.push(val.trim().to_string());
        }
    }
    (event_name, data_lines.join("\n"))
}

pub fn is_terminal_openrouter_done_event(event_name: Option<&str>, data_text: &str) -> bool {
    let name_match = match event_name {
        None | Some("data") | Some("done") => true,
        _ => false,
    };
    name_match && data_text.trim().to_uppercase() == "[DONE]"
}

fn delta_type_to_block_kind(delta_type: Option<&str>) -> Option<&'static str> {
    match delta_type {
        Some("thinking_delta") | Some("signature_delta") => Some("thinking"),
        Some("text_delta") => Some("text"),
        Some("input_json_delta") => Some("tool_use"),
        _ => None,
    }
}

fn synthetic_start_content_block(
    block_kind: &str,
    upstream_index: usize,
    stored_tool_block: Option<&Value>,
) -> Value {
    match block_kind {
        "tool_use" => {
            if let Some(stored) = stored_tool_block {
                if stored.get("type").and_then(|v| v.as_str()) == Some("tool_use") {
                    let default_id = format!("toolu_or_{}", upstream_index);
                let tool_id = stored
                        .get("id")
                        .and_then(|v| v.as_str())
                        .filter(|s| !s.is_empty())
                        .unwrap_or(&default_id);
                    let name = stored.get("name").and_then(|v| v.as_str()).unwrap_or("");
                    let inp = stored
                        .get("input")
                        .and_then(|v| v.as_object())
                        .map(|_| stored.get("input").unwrap().clone())
                        .unwrap_or(Value::Object(serde_json::Map::new()));
                    return serde_json::json!({
                        "type": "tool_use",
                        "id": tool_id,
                        "name": name,
                        "input": inp,
                    });
                }
            }
            serde_json::json!({
                "type": "tool_use",
                "id": format!("toolu_or_{}", upstream_index),
                "name": "",
                "input": {},
            })
        }
        "thinking" => serde_json::json!({
            "type": "thinking",
            "thinking": "",
        }),
        _ => serde_json::json!({
            "type": "text",
            "text": "",
        }),
    }
}

fn should_drop_block_type(block_type: Option<&str>, thinking_enabled: bool) -> bool {
    match block_type {
        Some(t) if t.starts_with("redacted_thinking") => !thinking_enabled,
        Some(t) if t.contains("thinking") && !thinking_enabled => true,
        _ => false,
    }
}

fn synthetic_close_other_open_blocks(
    state: &mut NativeSseBlockPolicyState,
    current_upstream: usize,
) -> String {
    let mut out = String::new();
    let upstreams: Vec<usize> = state.by_upstream.keys().copied().collect();
    for upstream in upstreams {
        if upstream == current_upstream {
            continue;
        }
        let seg = state.by_upstream.get_mut(&upstream);
        match seg {
            Some(seg) if seg.open => {
                out.push_str(&format_native_sse_event(
                    Some("content_block_stop"),
                    &serde_json::json!({
                        "type": "content_block_stop",
                        "index": seg.down_index,
                    })
                    .to_string(),
                ));
                seg.open = false;
                state.pending_suppressed_stops.insert(upstream);
            }
            _ => {}
        }
    }
    out
}

fn allocate_new_segment(
    state: &mut NativeSseBlockPolicyState,
    upstream_index: usize,
    block_type: &str,
    last_start_block: Option<Value>,
) -> usize {
    let new_idx = state.next_index;
    state.next_index += 1;
    state.by_upstream.insert(
        upstream_index,
        UpstreamBlockState {
            block_type: block_type.to_string(),
            down_index: new_idx,
            open: true,
            last_start_block,
        },
    );
    new_idx
}

pub fn transform_native_sse_block_event(
    event: &str,
    state: &mut NativeSseBlockPolicyState,
    thinking_enabled: bool,
) -> Option<String> {
    let (event_name, data_text) = parse_native_sse_event(event);
    let event_name = match event_name {
        Some(ref n) if !n.is_empty() => n.clone(),
        _ => return Some(event.to_string()),
    };
    if data_text.is_empty() {
        return Some(event.to_string());
    }

    let payload: Value = match serde_json::from_str(&data_text) {
        Ok(v) => v,
        Err(_) => return Some(event.to_string()),
    };

    if event_name == "content_block_start" {
        let block = match payload.get("content_block").and_then(|v| v.as_object()) {
            Some(b) => b,
            None => return Some(event.to_string()),
        };
        let block_type = block.get("type").and_then(|v| v.as_str());
        let upstream_index = match payload.get("index").and_then(|v| v.as_i64()) {
            Some(i) => i as usize,
            None => return Some(event.to_string()),
        };

        if should_drop_block_type(block_type, thinking_enabled) {
            state.dropped_indexes.insert(upstream_index);
            return None;
        }

        let block_type = match block_type {
            Some(t) => t,
            None => return Some(event.to_string()),
        };

        let prefix = synthetic_close_other_open_blocks(state, upstream_index);
        let stored = block
            .get("type")
            .map(|_| payload.get("content_block").unwrap().clone());
        let new_idx = allocate_new_segment(state, upstream_index, block_type, stored);

        let mut new_payload = payload.clone();
        if let Some(obj) = new_payload.as_object_mut() {
            obj.insert("index".to_string(), Value::Number((new_idx as i64).into()));
        }

        let result = prefix + &format_native_sse_event(Some(&event_name), &new_payload.to_string());
        return Some(result);
    }

    if event_name == "content_block_delta" {
        let delta = match payload.get("delta").and_then(|v| v.as_object()) {
            Some(d) => d,
            None => return Some(event.to_string()),
        };
        let delta_type = delta.get("type").and_then(|v| v.as_str());
        let upstream_index = match payload.get("index").and_then(|v| v.as_i64()) {
            Some(i) => i as usize,
            None => return Some(event.to_string()),
        };

        if state.dropped_indexes.contains(&upstream_index) {
            return None;
        }

        if should_drop_block_type(delta_type, thinking_enabled) {
            return None;
        }

        let block_kind = match delta_type_to_block_kind(delta_type) {
            Some(k) => k,
            None => return Some(event.to_string()),
        };

        let seg = state.by_upstream.get(&upstream_index);
        if let Some(seg) = seg {
            if seg.open {
                let mut new_payload = payload.clone();
                if let Some(obj) = new_payload.as_object_mut() {
                    obj.insert(
                        "index".to_string(),
                        Value::Number((seg.down_index as i64).into()),
                    );
                }
                return Some(format_native_sse_event(Some(&event_name), &new_payload.to_string()));
            }

            state.pending_suppressed_stops.remove(&upstream_index);
            let carry = seg.last_start_block.clone();
            let new_idx = allocate_new_segment(state, upstream_index, block_kind, carry.clone());

            let stored_tool = carry.as_ref().and_then(|c| {
                if c.get("type").and_then(|v| v.as_str()) == Some("tool_use") {
                    Some(c)
                } else {
                    None
                }
            });

            let start_payload = serde_json::json!({
                "type": "content_block_start",
                "index": new_idx,
                "content_block": synthetic_start_content_block(block_kind, upstream_index, stored_tool),
            });

            let mut new_payload = payload.clone();
            if let Some(obj) = new_payload.as_object_mut() {
                obj.insert(
                    "index".to_string(),
                    Value::Number((new_idx as i64).into()),
                );
            }

            return Some(
                format_native_sse_event(Some("content_block_start"), &start_payload.to_string())
                    + &format_native_sse_event(Some(&event_name), &new_payload.to_string()),
            );
        }

        if block_kind == "text" || block_kind == "tool_use" {
            let synthetic_block =
                synthetic_start_content_block(block_kind, upstream_index, None);
            let new_idx = allocate_new_segment(
                state,
                upstream_index,
                block_kind,
                Some(synthetic_block.clone()),
            );

            let start_payload = serde_json::json!({
                "type": "content_block_start",
                "index": new_idx,
                "content_block": synthetic_block,
            });

            let mut new_payload = payload.clone();
            if let Some(obj) = new_payload.as_object_mut() {
                obj.insert(
                    "index".to_string(),
                    Value::Number((new_idx as i64).into()),
                );
            }

            return Some(
                format_native_sse_event(Some("content_block_start"), &start_payload.to_string())
                    + &format_native_sse_event(Some(&event_name), &new_payload.to_string()),
            );
        }

        return Some(event.to_string());
    }

    if event_name == "content_block_stop" {
        let upstream_index = match payload.get("index").and_then(|v| v.as_i64()) {
            Some(i) => i as usize,
            None => return Some(event.to_string()),
        };

        if state.dropped_indexes.contains(&upstream_index) {
            return None;
        }

        if state.pending_suppressed_stops.contains(&upstream_index) {
            state.pending_suppressed_stops.remove(&upstream_index);
            return None;
        }

        if let Some(seg) = state.by_upstream.get_mut(&upstream_index) {
            if seg.open {
                let mut new_payload = payload.clone();
                if let Some(obj) = new_payload.as_object_mut() {
                    obj.insert(
                        "index".to_string(),
                        Value::Number((seg.down_index as i64).into()),
                    );
                }
                seg.open = false;
                return Some(format_native_sse_event(Some(&event_name), &new_payload.to_string()));
            }
            return None;
        }

        if !thinking_enabled {
            return None;
        }

        return Some(event.to_string());
    }

    Some(event.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_native_sse_event() {
        let result = format_native_sse_event(Some("ping"), "{\"type\":\"ping\"}");
        assert_eq!(result, "event: ping\ndata: {\"type\":\"ping\"}\n\n");
    }

    #[test]
    fn test_format_native_sse_event_no_name() {
        let result = format_native_sse_event(None, "{\"key\":\"val\"}");
        assert_eq!(result, "data: {\"key\":\"val\"}\n\n");
    }

    #[test]
    fn test_format_native_sse_event_multiline_data() {
        let result = format_native_sse_event(Some("test"), "line1\nline2");
        assert!(result.contains("data: line1"));
        assert!(result.contains("data: line2"));
    }

    #[test]
    fn test_parse_native_sse_event() {
        let event = "event: message_start\ndata: {\"type\":\"message_start\"}";
        let (name, data) = parse_native_sse_event(event);
        assert_eq!(name, Some("message_start".to_string()));
        assert_eq!(data, "{\"type\":\"message_start\"}");
    }

    #[test]
    fn test_parse_native_sse_event_data_only() {
        let event = "data: {\"key\":\"val\"}";
        let (name, data) = parse_native_sse_event(event);
        assert_eq!(name, None);
        assert_eq!(data, "{\"key\":\"val\"}");
    }

    #[test]
    fn test_is_terminal_openrouter_done_event() {
        assert!(is_terminal_openrouter_done_event(None, "[DONE]"));
        assert!(is_terminal_openrouter_done_event(Some("data"), "[DONE]"));
        assert!(is_terminal_openrouter_done_event(Some("done"), "[DONE]"));
        assert!(!is_terminal_openrouter_done_event(Some("message_start"), "[DONE]"));
        assert!(!is_terminal_openrouter_done_event(None, "not done"));
    }

    #[test]
    fn test_should_drop_block_type_thinking_disabled() {
        assert!(should_drop_block_type(Some("thinking"), false));
        assert!(!should_drop_block_type(Some("thinking"), true));
        assert!(should_drop_block_type(Some("redacted_thinking"), false));
        assert!(!should_drop_block_type(Some("text"), false));
        assert!(!should_drop_block_type(Some("tool_use"), false));
    }

    #[test]
    fn test_state_new() {
        let state = NativeSseBlockPolicyState::new();
        assert_eq!(state.next_index, 0);
        assert!(state.by_upstream.is_empty());
        assert!(state.dropped_indexes.is_empty());
        assert!(state.pending_suppressed_stops.is_empty());
        assert!(!state.message_stopped);
    }

    #[test]
    fn test_transform_content_block_start() {
        let mut state = NativeSseBlockPolicyState::new();
        let event = "event: content_block_start\ndata: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"text\",\"text\":\"\"}}\n\n";
        let result = transform_native_sse_block_event(event, &mut state, true);
        assert!(result.is_some());
        let output = result.unwrap();
        assert!(output.contains("event: content_block_start"));
        assert!(output.contains("\"index\":0"));
        assert_eq!(state.next_index, 1);
    }

    #[test]
    fn test_transform_drop_thinking_block() {
        let mut state = NativeSseBlockPolicyState::new();
        let event = "event: content_block_start\ndata: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"thinking\",\"thinking\":\"\"}}\n\n";
        let result = transform_native_sse_block_event(event, &mut state, false);
        assert!(result.is_none());
        assert!(state.dropped_indexes.contains(&0));
    }

    #[test]
    fn test_transform_content_block_delta() {
        let mut state = NativeSseBlockPolicyState::new();
        let start = "event: content_block_start\ndata: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"text\",\"text\":\"\"}}\n\n";
        transform_native_sse_block_event(start, &mut state, true);

        let delta = "event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"Hello\"}}\n\n";
        let result = transform_native_sse_block_event(delta, &mut state, true);
        assert!(result.is_some());
        let output = result.unwrap();
        assert!(output.contains("\"index\":0"));
    }

    #[test]
    fn test_transform_content_block_stop() {
        let mut state = NativeSseBlockPolicyState::new();
        let start = "event: content_block_start\ndata: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"text\",\"text\":\"\"}}\n\n";
        transform_native_sse_block_event(start, &mut state, true);

        let stop = "event: content_block_stop\ndata: {\"type\":\"content_block_stop\",\"index\":0}\n\n";
        let result = transform_native_sse_block_event(stop, &mut state, true);
        assert!(result.is_some());
        let output = result.unwrap();
        assert!(output.contains("\"index\":0"));
    }

    #[test]
    fn test_transform_duplicate_stop_suppressed() {
        let mut state = NativeSseBlockPolicyState::new();
        let start = "event: content_block_start\ndata: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"text\",\"text\":\"\"}}\n\n";
        transform_native_sse_block_event(start, &mut state, true);
        let stop = "event: content_block_stop\ndata: {\"type\":\"content_block_stop\",\"index\":0}\n\n";
        transform_native_sse_block_event(stop, &mut state, true);

        let result = transform_native_sse_block_event(stop, &mut state, true);
        assert!(result.is_none());
    }

    #[test]
    fn test_transform_dropped_index_delta_suppressed() {
        let mut state = NativeSseBlockPolicyState::new();
        let start = "event: content_block_start\ndata: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"thinking\",\"thinking\":\"\"}}\n\n";
        transform_native_sse_block_event(start, &mut state, false);

        let delta = "event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"thinking_delta\",\"thinking\":\"test\"}}\n\n";
        let result = transform_native_sse_block_event(delta, &mut state, false);
        assert!(result.is_none());
    }
}
