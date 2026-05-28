use serde_json::Value;
use std::collections::HashSet;

use super::server_tool_sse::{
    WEB_FETCH_TOOL_RESULT, WEB_SEARCH_TOOL_RESULT, SERVER_TOOL_USE,
};

#[derive(Debug, Clone)]
pub struct SSEEvent {
    pub event: String,
    pub data: Value,
    pub raw: String,
}

fn no_delta_block_kinds() -> HashSet<&'static str> {
    HashSet::from([
        SERVER_TOOL_USE,
        WEB_SEARCH_TOOL_RESULT,
        WEB_FETCH_TOOL_RESULT,
        "text_eager",
        "redacted_thinking",
    ])
}

fn allowed_block_start_types() -> HashSet<&'static str> {
    HashSet::from([
        "text",
        "thinking",
        "tool_use",
        "redacted_thinking",
        SERVER_TOOL_USE,
        WEB_SEARCH_TOOL_RESULT,
        WEB_FETCH_TOOL_RESULT,
    ])
}

pub fn event_index(event: &SSEEvent) -> usize {
    event
        .data
        .get("index")
        .and_then(|v| v.as_i64())
        .expect("event must have an index field") as usize
}

pub fn parse_sse_lines(lines: &[&str]) -> Vec<SSEEvent> {
    let mut events = Vec::new();
    let mut current_event = String::new();
    let mut data_parts: Vec<String> = Vec::new();
    let mut raw_parts: Vec<String> = Vec::new();

    for line in lines {
        let stripped = line.trim_end_matches('\r').trim_end_matches('\n');
        if stripped.is_empty() {
            append_event(&mut events, &current_event, &data_parts, &raw_parts);
            current_event.clear();
            data_parts.clear();
            raw_parts.clear();
            continue;
        }
        raw_parts.push(stripped.to_string());
        if let Some(val) = stripped.strip_prefix("event:") {
            current_event = val.trim().to_string();
        } else if let Some(val) = stripped.strip_prefix("data:") {
            data_parts.push(val.trim().to_string());
        }
    }

    append_event(&mut events, &current_event, &data_parts, &raw_parts);
    events
}

pub fn parse_sse_text(text: &str) -> Vec<SSEEvent> {
    let lines: Vec<&str> = text.lines().collect();
    parse_sse_lines(&lines)
}

fn append_event(
    events: &mut Vec<SSEEvent>,
    current_event: &str,
    data_parts: &[String],
    raw_parts: &[String],
) {
    if current_event.is_empty() && data_parts.is_empty() {
        return;
    }
    let data_text = data_parts.join("\n");
    let data: Value = if data_text.is_empty() {
        Value::Object(serde_json::Map::new())
    } else {
        serde_json::from_str(&data_text).unwrap_or_else(|_| {
            let mut map = serde_json::Map::new();
            map.insert("raw".to_string(), Value::String(data_text));
            Value::Object(map)
        })
    };
    events.push(SSEEvent {
        event: current_event.to_string(),
        data,
        raw: raw_parts.join("\n"),
    });
}

pub fn assert_anthropic_stream_contract(events: &[SSEEvent], allow_error: bool) {
    assert!(!events.is_empty(), "stream produced no SSE events");
    let event_names: Vec<&str> = events.iter().map(|e| e.event.as_str()).collect();
    assert!(
        event_names.contains(&"message_start"),
        "missing message_start in events: {:?}",
        event_names
    );
    assert_eq!(
        event_names[event_names.len() - 1],
        "message_stop",
        "last event must be message_stop, got: {:?}",
        event_names
    );

    let no_delta = no_delta_block_kinds();
    let allowed_starts = allowed_block_start_types();
    let mut open_blocks: std::collections::HashMap<usize, String> =
        std::collections::HashMap::new();
    let mut seen_blocks: std::collections::HashSet<usize> = std::collections::HashSet::new();

    for event in events {
        if event.event == "error" && !allow_error {
            panic!("unexpected SSE error event: {:?}", event.data);
        }

        if event.event == "content_block_start" {
            let index = event_index(event);
            let block = event
                .data
                .get("content_block")
                .and_then(|v| v.as_object())
                .expect("content_block_start must have a content_block object");
            let block_type = block
                .get("type")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            assert!(
                allowed_starts.contains(block_type.as_str()),
                "unexpected block type: {}",
                block_type
            );
            assert!(
                !open_blocks.contains_key(&index),
                "block {} started twice",
                index
            );
            assert!(
                !seen_blocks.contains(&index),
                "block {} reused after stop",
                index
            );
            let storage = if block_type == "text"
                && block
                    .get("text")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .trim()
                    .is_empty()
            {
                "text_eager"
            } else {
                &block_type
            };
            open_blocks.insert(index, storage.to_string());
            seen_blocks.insert(index);
            continue;
        }

        if event.event == "content_block_delta" {
            let index = event_index(event);
            assert!(
                open_blocks.contains_key(&index),
                "delta for unopened block {}",
                index
            );
            let kind = open_blocks.get(&index).unwrap();
            assert!(
                !no_delta.contains(kind.as_str()),
                "unexpected delta for start/stop-only block {} at index {}",
                kind,
                index
            );
            let delta = event
                .data
                .get("delta")
                .and_then(|v| v.as_object())
                .expect("content_block_delta must have a delta object");
            let delta_type = delta
                .get("type")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if kind == "thinking" {
                assert!(
                    delta_type == "thinking_delta" || delta_type == "signature_delta",
                    "block {} is thinking, got delta_type {}",
                    index,
                    delta_type
                );
                continue;
            }
            let expected = match kind.as_str() {
                "text" => "text_delta",
                "tool_use" => "input_json_delta",
                _ => panic!("unexpected block kind: {}", kind),
            };
            assert_eq!(
                delta_type, expected,
                "block {} is {}, got delta_type {}",
                index, kind, delta_type
            );
            continue;
        }

        if event.event == "content_block_stop" {
            let index = event_index(event);
            assert!(
                open_blocks.contains_key(&index),
                "stop for unopened block {}",
                index
            );
            open_blocks.remove(&index);
        }
    }

    assert!(
        open_blocks.is_empty(),
        "unclosed blocks: {:?}",
        open_blocks
    );
    assert!(!seen_blocks.is_empty(), "stream did not emit any content blocks");
}

pub fn event_names(events: &[SSEEvent]) -> Vec<String> {
    events.iter().map(|e| e.event.clone()).collect()
}

pub fn text_content(events: &[SSEEvent]) -> String {
    let mut parts = Vec::new();
    for event in events {
        if event.event == "content_block_start" {
            if let Some(block) = event.data.get("content_block").and_then(|v| v.as_object()) {
                if block.get("type").and_then(|v| v.as_str()) == Some("text") {
                    if let Some(text) = block.get("text").and_then(|v| v.as_str()) {
                        if !text.is_empty() {
                            parts.push(text);
                        }
                    }
                }
            }
        }
        if let Some(delta) = event.data.get("delta").and_then(|v| v.as_object()) {
            if delta.get("type").and_then(|v| v.as_str()) == Some("text_delta") {
                if let Some(text) = delta.get("text").and_then(|v| v.as_str()) {
                    parts.push(text);
                }
            }
        }
    }
    parts.join("")
}

pub fn thinking_content(events: &[SSEEvent]) -> String {
    let mut parts = Vec::new();
    for event in events {
        if let Some(delta) = event.data.get("delta").and_then(|v| v.as_object()) {
            if delta.get("type").and_then(|v| v.as_str()) == Some("thinking_delta") {
                if let Some(text) = delta.get("thinking").and_then(|v| v.as_str()) {
                    parts.push(text);
                }
            }
        }
    }
    parts.join("")
}

pub fn has_tool_use(events: &[SSEEvent]) -> bool {
    for event in events {
        if let Some(block) = event.data.get("content_block").and_then(|v| v.as_object()) {
            if block.get("type").and_then(|v| v.as_str()) == Some("tool_use") {
                return true;
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_sse_lines_single_event() {
        let lines = vec!["event: message_start", "data: {\"type\":\"message_start\",\"message\":{\"id\":\"msg_1\",\"model\":\"claude-3\"}}", ""];
        let events = parse_sse_lines(&lines);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event, "message_start");
        assert_eq!(events[0].data["message"]["id"].as_str(), Some("msg_1"));
    }

    #[test]
    fn test_parse_sse_lines_multiple_events() {
        let lines = vec![
            "event: message_start",
            "data: {\"type\":\"message_start\"}",
            "",
            "event: content_block_start",
            "data: {\"type\":\"content_block_start\",\"index\":0}",
            "",
            "event: message_stop",
            "data: {\"type\":\"message_stop\"}",
            "",
        ];
        let events = parse_sse_lines(&lines);
        assert_eq!(events.len(), 3);
        assert_eq!(events[0].event, "message_start");
        assert_eq!(events[1].event, "content_block_start");
        assert_eq!(events[2].event, "message_stop");
    }

    #[test]
    fn test_parse_sse_lines_data_only() {
        let lines = vec!["data: {\"type\":\"ping\"}", ""];
        let events = parse_sse_lines(&lines);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event, "");
        assert_eq!(events[0].data["type"].as_str(), Some("ping"));
    }

    #[test]
    fn test_parse_sse_text() {
        let text = "event: message_start\ndata: {\"type\":\"message_start\"}\n\nevent: message_stop\ndata: {\"type\":\"message_stop\"}\n\n";
        let events = parse_sse_text(text);
        assert_eq!(events.len(), 2);
    }

    #[test]
    fn test_event_index() {
        let event = SSEEvent {
            event: "content_block_start".to_string(),
            data: serde_json::json!({"type": "content_block_start", "index": 3}),
            raw: String::new(),
        };
        assert_eq!(event_index(&event), 3);
    }

    #[test]
    fn test_text_content() {
        let events = vec![
            SSEEvent {
                event: "content_block_start".to_string(),
                data: serde_json::json!({"type": "content_block_start", "index": 0, "content_block": {"type": "text", "text": ""}}),
                raw: String::new(),
            },
            SSEEvent {
                event: "content_block_delta".to_string(),
                data: serde_json::json!({"type": "content_block_delta", "index": 0, "delta": {"type": "text_delta", "text": "Hello"}}),
                raw: String::new(),
            },
            SSEEvent {
                event: "content_block_delta".to_string(),
                data: serde_json::json!({"type": "content_block_delta", "index": 0, "delta": {"type": "text_delta", "text": " world"}}),
                raw: String::new(),
            },
        ];
        assert_eq!(text_content(&events), "Hello world");
    }

    #[test]
    fn test_thinking_content() {
        let events = vec![
            SSEEvent {
                event: "content_block_delta".to_string(),
                data: serde_json::json!({"type": "content_block_delta", "index": 0, "delta": {"type": "thinking_delta", "thinking": "I think"}}),
                raw: String::new(),
            },
            SSEEvent {
                event: "content_block_delta".to_string(),
                data: serde_json::json!({"type": "content_block_delta", "index": 0, "delta": {"type": "thinking_delta", "thinking": " therefore"}}),
                raw: String::new(),
            },
        ];
        assert_eq!(thinking_content(&events), "I think therefore");
    }

    #[test]
    fn test_has_tool_use_true() {
        let events = vec![SSEEvent {
            event: "content_block_start".to_string(),
            data: serde_json::json!({"type": "content_block_start", "index": 0, "content_block": {"type": "tool_use", "id": "toolu_1", "name": "test", "input": {}}}),
            raw: String::new(),
        }];
        assert!(has_tool_use(&events));
    }

    #[test]
    fn test_has_tool_use_false() {
        let events = vec![SSEEvent {
            event: "content_block_start".to_string(),
            data: serde_json::json!({"type": "content_block_start", "index": 0, "content_block": {"type": "text", "text": "hello"}}),
            raw: String::new(),
        }];
        assert!(!has_tool_use(&events));
    }

    #[test]
    fn test_event_names() {
        let events = vec![
            SSEEvent {
                event: "message_start".to_string(),
                data: serde_json::json!({"type": "message_start"}),
                raw: String::new(),
            },
            SSEEvent {
                event: "message_stop".to_string(),
                data: serde_json::json!({"type": "message_stop"}),
                raw: String::new(),
            },
        ];
        assert_eq!(event_names(&events), vec!["message_start", "message_stop"]);
    }

    #[test]
    fn test_append_event_invalid_json() {
        let lines = vec!["data: not-valid-json", ""];
        let events = parse_sse_lines(&lines);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].data["raw"].as_str(), Some("not-valid-json"));
    }
}
