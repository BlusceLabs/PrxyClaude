use serde_json::Value;
use crate::core::anthropic::stream_contracts::{parse_sse_lines, SSEEvent};

pub struct EmittedNativeSseTracker {
    buf: String,
    open_stack: Vec<usize>,
    max_index: i32,
    pub message_id: Option<String>,
    pub model: String,
}

impl EmittedNativeSseTracker {
    pub fn new() -> Self {
        Self {
            buf: String::new(),
            open_stack: Vec::new(),
            max_index: -1,
            message_id: None,
            model: String::new(),
        }
    }

    pub fn feed(&mut self, chunk: &str) {
        self.buf.push_str(chunk);
        loop {
            let sep = self.buf.find("\n\n");
            if sep.is_none() {
                break;
            }
            let sep = sep.unwrap();
            let frame = self.buf[..sep].to_string();
            self.buf = self.buf[sep + 2..].to_string();
            if frame.trim().is_empty() {
                continue;
            }
            for event in parse_sse_lines(&frame.lines().collect::<Vec<&str>>()) {
                self.observe(&event);
            }
        }
    }

    pub fn feed_value(&mut self, value: &Value) {
        let event_type = value.get("type").and_then(|v| v.as_str()).unwrap_or("");
        if event_type == "message_start" {
            if let Some(message) = value.get("message").and_then(|v| v.as_object()) {
                if let Some(mid) = message.get("id").and_then(|v| v.as_str()) {
                    if !mid.is_empty() {
                        self.message_id = Some(mid.to_string());
                    }
                }
                if let Some(model) = message.get("model").and_then(|v| v.as_str()) {
                    if !model.is_empty() {
                        self.model = model.to_string();
                    }
                }
            }
        } else if event_type == "content_block_start" {
            if let Some(idx) = value.get("index").and_then(|v| v.as_i64()) {
                let idx = idx as usize;
                self.max_index = self.max_index.max(idx as i32);
                self.open_stack.push(idx);
            }
        } else if event_type == "content_block_stop" {
            if let Some(idx) = value.get("index").and_then(|v| v.as_i64()) {
                let idx = idx as usize;
                if let Some(pos) = self.open_stack.iter().rposition(|&i| i == idx) {
                    self.open_stack.remove(pos);
                }
            }
        }
    }

    fn observe(&mut self, event: &SSEEvent) {
        if event.event == "message_start" {
            if let Some(message) = event.data.get("message").and_then(|v| v.as_object()) {
                if let Some(mid) = message.get("id").and_then(|v| v.as_str()) {
                    if !mid.is_empty() {
                        self.message_id = Some(mid.to_string());
                    }
                }
                if let Some(model) = message.get("model").and_then(|v| v.as_str()) {
                    if !model.is_empty() {
                        self.model = model.to_string();
                    }
                }
            }
        } else if event.event == "content_block_start" {
            let idx = crate::core::anthropic::stream_contracts::event_index(event);
            self.max_index = self.max_index.max(idx as i32);
            self.open_stack.push(idx);
        } else if event.event == "content_block_stop" {
            let idx = crate::core::anthropic::stream_contracts::event_index(event);
            if let Some(pos) = self.open_stack.iter().rposition(|&i| i == idx) {
                self.open_stack.remove(pos);
            }
        }
    }

    pub fn next_content_index(&self) -> usize {
        (self.max_index + 1) as usize
    }

    pub fn close_unclosed_blocks(&mut self) -> Vec<Value> {
        let mut events = Vec::new();
        while let Some(idx) = self.open_stack.pop() {
            events.push(serde_json::json!({
                "type": "content_block_stop",
                "index": idx,
            }));
        }
        events
    }

    pub fn midstream_error_tail(
        &mut self,
        error_message: &str,
        input_tokens: i32,
    ) -> Vec<Value> {
        let next_idx = self.next_content_index();

        let mut events = Vec::new();

        events.push(serde_json::json!({
            "type": "content_block_start",
            "index": next_idx,
            "content_block": {
                "type": "text",
                "text": "",
            },
        }));

        events.push(serde_json::json!({
            "type": "content_block_delta",
            "index": next_idx,
            "delta": {
                "type": "text_delta",
                "text": error_message,
            },
        }));

        events.push(serde_json::json!({
            "type": "content_block_stop",
            "index": next_idx,
        }));

        events.push(serde_json::json!({
            "type": "message_delta",
            "delta": {
                "stop_reason": "end_turn",
                "stop_sequence": null,
            },
            "usage": {
                "input_tokens": input_tokens,
                "output_tokens": 1,
            },
        }));

        events.push(serde_json::json!({
            "type": "message_stop",
        }));

        events
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_tracker() {
        let tracker = EmittedNativeSseTracker::new();
        assert_eq!(tracker.max_index, -1);
        assert!(tracker.open_stack.is_empty());
        assert!(tracker.message_id.is_none());
        assert!(tracker.model.is_empty());
    }

    #[test]
    fn test_feed_message_start() {
        let mut tracker = EmittedNativeSseTracker::new();
        let chunk = "event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"id\":\"msg_123\",\"model\":\"claude-3\"}}\n\n";
        tracker.feed(chunk);
        assert_eq!(tracker.message_id, Some("msg_123".to_string()));
        assert_eq!(tracker.model, "claude-3");
    }

    #[test]
    fn test_feed_content_block_start_stop() {
        let mut tracker = EmittedNativeSseTracker::new();
        tracker.feed("event: content_block_start\ndata: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"text\",\"text\":\"\"}}\n\n");
        assert_eq!(tracker.open_stack.len(), 1);
        assert_eq!(tracker.max_index, 0);

        tracker.feed("event: content_block_stop\ndata: {\"type\":\"content_block_stop\",\"index\":0}\n\n");
        assert!(tracker.open_stack.is_empty());
    }

    #[test]
    fn test_next_content_index() {
        let mut tracker = EmittedNativeSseTracker::new();
        tracker.feed("event: content_block_start\ndata: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"text\",\"text\":\"\"}}\n\n");
        tracker.feed("event: content_block_start\ndata: {\"type\":\"content_block_start\",\"index\":2,\"content_block\":{\"type\":\"text\",\"text\":\"\"}}\n\n");
        assert_eq!(tracker.next_content_index(), 3);
    }

    #[test]
    fn test_close_unclosed_blocks() {
        let mut tracker = EmittedNativeSseTracker::new();
        tracker.feed("event: content_block_start\ndata: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"text\",\"text\":\"\"}}\n\n");
        tracker.feed("event: content_block_start\ndata: {\"type\":\"content_block_start\",\"index\":1,\"content_block\":{\"type\":\"text\",\"text\":\"\"}}\n\n");

        let events = tracker.close_unclosed_blocks();
        assert_eq!(events.len(), 2);
        assert!(tracker.open_stack.is_empty());
    }

    #[test]
    fn test_feed_value_message_start() {
        let mut tracker = EmittedNativeSseTracker::new();
        tracker.feed_value(&serde_json::json!({
            "type": "message_start",
            "message": {"id": "msg_456", "model": "claude-opus"}
        }));
        assert_eq!(tracker.message_id, Some("msg_456".to_string()));
        assert_eq!(tracker.model, "claude-opus");
    }

    #[test]
    fn test_feed_value_content_blocks() {
        let mut tracker = EmittedNativeSseTracker::new();
        tracker.feed_value(&serde_json::json!({
            "type": "content_block_start",
            "index": 0,
            "content_block": {"type": "text", "text": ""}
        }));
        tracker.feed_value(&serde_json::json!({
            "type": "content_block_start",
            "index": 1,
            "content_block": {"type": "text", "text": ""}
        }));
        assert_eq!(tracker.open_stack.len(), 2);
        assert_eq!(tracker.next_content_index(), 2);

        tracker.feed_value(&serde_json::json!({
            "type": "content_block_stop",
            "index": 0,
        }));
        assert_eq!(tracker.open_stack.len(), 1);

        tracker.feed_value(&serde_json::json!({
            "type": "content_block_stop",
            "index": 1,
        }));
        assert!(tracker.open_stack.is_empty());
    }

    #[test]
    fn test_midstream_error_tail() {
        let mut tracker = EmittedNativeSseTracker::new();
        let events = tracker.midstream_error_tail("Something went wrong", 42);
        assert_eq!(events.len(), 5);

        assert_eq!(events[0]["type"].as_str(), Some("content_block_start"));
        assert_eq!(events[0]["index"].as_i64(), Some(0));
        assert_eq!(events[1]["type"].as_str(), Some("content_block_delta"));
        assert_eq!(
            events[1]["delta"]["text"].as_str(),
            Some("Something went wrong")
        );
        assert_eq!(events[2]["type"].as_str(), Some("content_block_stop"));
        assert_eq!(events[3]["type"].as_str(), Some("message_delta"));
        assert_eq!(
            events[3]["delta"]["stop_reason"].as_str(),
            Some("end_turn")
        );
        assert_eq!(events[4]["type"].as_str(), Some("message_stop"));
    }
}
