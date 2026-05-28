use futures::StreamExt;
use serde_json::Value;
use tokio_stream::wrappers::UnboundedReceiverStream;

use crate::providers::ProviderStream;

pub fn format_sse_event(event_type: &str, data: &Value) -> String {
    format!("event: {}\ndata: {}\n\n", event_type, data)
}

pub struct SSEBuilder {
    events: Vec<String>,
}

impl SSEBuilder {
    pub fn new() -> Self {
        Self { events: Vec::new() }
    }

    pub fn add_event(mut self, event_type: &str, data: Value) -> Self {
        self.events.push(format_sse_event(event_type, &data));
        self
    }

    pub fn build(self) -> String {
        self.events.join("")
    }
}

pub struct ContentBlockManager {
    blocks: Vec<Value>,
    current_index: usize,
}

impl ContentBlockManager {
    pub fn new() -> Self {
        Self {
            blocks: Vec::new(),
            current_index: 0,
        }
    }

    pub fn add_block(&mut self, block: Value) {
        self.blocks.push(block);
    }

    pub fn get_block(&mut self, index: usize) -> Option<&Value> {
        self.blocks.get(index)
    }

    pub fn next_block(&mut self) -> Option<&Value> {
        let block = self.blocks.get(self.current_index);
        if block.is_some() {
            self.current_index += 1;
        }
        block
    }
}

pub fn map_stop_reason(stop_reason: &str) -> Option<String> {
    match stop_reason {
        "end_turn" => Some("end_turn".to_string()),
        "max_tokens" => Some("max_tokens".to_string()),
        "stop_sequence" => Some("stop_sequence".to_string()),
        "tool_use" => Some("tool_use".to_string()),
        _ => None,
    }
}

fn estimate_tokens(text: &str) -> i32 {
    (text.len() as f64 / 4.0).ceil() as i32
}

/// Wrap a raw provider stream to ensure all chunks are in Anthropic SSE event format.
///
/// Auto-detects format:
/// - Values with a `"type"` field (Anthropic-native) pass through unchanged.
/// - Values without `"type"` are treated as OpenAI delta chunks and converted.
pub fn to_anthropic_format(rx: tokio::sync::mpsc::UnboundedReceiver<Value>) -> ProviderStream {
    let (tx, new_rx) = tokio::sync::mpsc::unbounded_channel();

    tokio::spawn(async move {
        let mut stream = UnboundedReceiverStream::new(rx);
        let mut converter = OpenAIToAnthropicConverter::new(tx.clone());

        while let Some(value) = stream.next().await {
            if !converter.process(value) {
                break;
            }
        }
        converter.finalize();
    });

    ProviderStream::new(new_rx)
}

struct OpenAIToAnthropicConverter {
    tx: tokio::sync::mpsc::UnboundedSender<Value>,
    message_id: String,
    model: String,
    content_started: bool,
    finished: bool,
    input_tokens: i32,
    output_tokens: i32,
}

impl OpenAIToAnthropicConverter {
    fn new(tx: tokio::sync::mpsc::UnboundedSender<Value>) -> Self {
        Self {
            tx,
            message_id: String::new(),
            model: String::new(),
            content_started: false,
            finished: false,
            input_tokens: 0,
            output_tokens: 0,
        }
    }

    fn send(&self, value: Value) -> bool {
        self.tx.send(value).is_ok()
    }

    fn process(&mut self, value: Value) -> bool {
        if self.finished {
            return true;
        }

        // Error values from providers
        if value.get("error").is_some() && value.get("type").is_none() {
            return self.send(serde_json::json!({
                "type": "error",
                "error": value.get("error"),
            }));
        }

        // Anthropic-native format: has "type" field, pass through
        if value.get("type").and_then(|v| v.as_str()).is_some() {
            return self.send(value);
        }

        // OpenAI delta format: convert
        self.process_openai_chunk(value)
    }

    fn process_openai_chunk(&mut self, chunk: Value) -> bool {
        let choices = match chunk.get("choices").and_then(|c| c.as_array()) {
            Some(c) => c,
            None => return true,
        };

        let choice = match choices.first() {
            Some(c) => c,
            None => return true,
        };

        let delta = choice.get("delta");
        let finish_reason = choice.get("finish_reason").and_then(|v| v.as_str());

        if !self.content_started {
            self.message_id = chunk
                .get("id")
                .and_then(|v| v.as_str())
                .unwrap_or("msg_unknown")
                .to_string();
            self.model = chunk
                .get("model")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string();

            if let Some(usage) = chunk.get("usage") {
                self.input_tokens = usage
                    .get("prompt_tokens")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(0) as i32;
            }

            if !self.send(serde_json::json!({
                "type": "message_start",
                "message": {
                    "id": self.message_id,
                    "type": "message",
                    "role": "assistant",
                    "content": [],
                    "model": self.model,
                    "stop_reason": null,
                    "stop_sequence": null,
                    "usage": {
                        "input_tokens": self.input_tokens,
                        "output_tokens": 0,
                    },
                },
            })) {
                return false;
            }

            if !self.send(serde_json::json!({
                "type": "content_block_start",
                "index": 0,
                "content_block": {
                    "type": "text",
                    "text": "",
                },
            })) {
                return false;
            }

            self.content_started = true;
        }

        if let Some(content) = delta.and_then(|d| d.get("content")).and_then(|v| v.as_str()) {
            if !content.is_empty() {
                self.output_tokens += estimate_tokens(content);

                if !self.send(serde_json::json!({
                    "type": "content_block_delta",
                    "index": 0,
                    "delta": {
                        "type": "text_delta",
                        "text": content,
                    },
                })) {
                    return false;
                }
            }
        }

        if let Some(reason) = finish_reason {
            self.finish(reason)
        } else {
            true
        }
    }

    fn finish(&mut self, reason: &str) -> bool {
        self.finished = true;
        let anthropic_stop_reason = match reason {
            "stop" => "end_turn",
            "length" => "max_tokens",
            "content_filter" => "stop_sequence",
            other => other,
        };

        if !self.send(serde_json::json!({
            "type": "content_block_stop",
            "index": 0,
        })) {
            return false;
        }

        if !self.send(serde_json::json!({
            "type": "message_delta",
            "delta": {
                "stop_reason": anthropic_stop_reason,
                "stop_sequence": null,
            },
            "usage": {
                "output_tokens": self.output_tokens,
            },
        })) {
            return false;
        }

        self.send(serde_json::json!({
            "type": "message_stop",
        }))
    }

    fn finalize(&mut self) {
        if !self.finished && self.content_started {
            self.finish("end_turn");
        }
    }
}

/// Parse SSE frames from a string buffer, extracting JSON `data:` values.
///
/// Handles SSE framing (`data: {...}` lines separated by `\n\n`), drops `[DONE]`
/// frames, and returns remaining unparsed buffer plus any parsed JSON values.
pub fn parse_sse_frames(buf: &str) -> (String, Vec<serde_json::Value>) {
    let mut remaining = buf.to_string();
    let mut values = Vec::new();

    while let Some(pos) = remaining.find("\n\n") {
        let frame = remaining[..pos].to_string();
        remaining = remaining[pos + 2..].to_string();

        for line in frame.lines() {
            if let Some(data) = line.strip_prefix("data: ") {
                if data != "[DONE]" {
                    if let Ok(value) = serde_json::from_str(data) {
                        values.push(value);
                    }
                }
            }
        }
    }

    (remaining, values)
}

/// Parse an SSE response stream into a ProviderStream of parsed JSON Values.
///
/// Handles SSE framing (`data: {...}` lines), drops `[DONE]` frames,
/// and wraps errors in a consistent `{"type": "error", ...}` envelope.
pub fn parse_sse_response(response: reqwest::Response) -> ProviderStream {
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();

    tokio::spawn(async move {
        let mut stream = response.bytes_stream();
        let mut buf = String::new();

        while let Some(chunk_result) = stream.next().await {
            match chunk_result {
                Ok(chunk) => {
                    buf.push_str(&String::from_utf8_lossy(&chunk));

                    let (new_buf, values) = parse_sse_frames(&buf);
                    buf = new_buf;
                    for value in values {
                        if tx.send(value).is_err() {
                            return;
                        }
                    }
                }
                Err(e) => {
                    let _ = tx.send(serde_json::json!({
                        "type": "error",
                        "error": {
                            "type": "stream_error",
                            "message": e.to_string()
                        }
                    }));
                    break;
                }
            }
        }
    });

    ProviderStream::new(rx)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_sse_event() {
        let data = serde_json::json!({"type": "ping"});
        let result = format_sse_event("ping", &data);
        assert_eq!(result, "event: ping\ndata: {\"type\":\"ping\"}\n\n");
    }

    #[test]
    fn test_map_stop_reason() {
        assert_eq!(map_stop_reason("end_turn"), Some("end_turn".to_string()));
        assert_eq!(map_stop_reason("unknown"), None);
    }

    #[test]
    fn test_estimate_tokens() {
        assert_eq!(estimate_tokens("hello"), 2);
        assert_eq!(estimate_tokens(""), 0);
    }

    #[test]
    fn test_parse_sse_frames_single() {
        let input = "data: {\"key\": \"value\"}\n\n";
        let (remaining, values) = parse_sse_frames(input);
        assert_eq!(remaining, "");
        assert_eq!(values.len(), 1);
        assert_eq!(values[0].get("key").and_then(|v| v.as_str()), Some("value"));
    }

    #[test]
    fn test_parse_sse_frames_multiple() {
        let input = "data: {\"a\":1}\n\ndata: {\"b\":2}\n\n";
        let (remaining, values) = parse_sse_frames(input);
        assert_eq!(remaining, "");
        assert_eq!(values.len(), 2);
    }

    #[test]
    fn test_parse_sse_frames_done_skipped() {
        let input = "data: {\"a\":1}\n\ndata: [DONE]\n\ndata: {\"b\":2}\n\n";
        let (remaining, values) = parse_sse_frames(input);
        assert_eq!(remaining, "");
        assert_eq!(values.len(), 2);
    }

    #[test]
    fn test_parse_sse_frames_partial_frame() {
        let input = "data: {\"key\": \"value\"}\n\nsome_remaining";
        let (remaining, values) = parse_sse_frames(input);
        assert_eq!(remaining, "some_remaining");
        assert_eq!(values.len(), 1);
    }

    #[test]
    fn test_parse_sse_frames_no_delimiter() {
        let input = "data: {\"key\": \"value\"}";
        let (remaining, values) = parse_sse_frames(input);
        assert_eq!(remaining, "data: {\"key\": \"value\"}");
        assert_eq!(values.len(), 0);
    }

    #[test]
    fn test_parse_sse_frames_empty() {
        let (remaining, values) = parse_sse_frames("");
        assert_eq!(remaining, "");
        assert!(values.is_empty());
    }

    #[test]
    fn test_parse_sse_frames_invalid_json() {
        let input = "data: not-json\n\n";
        let (remaining, values) = parse_sse_frames(input);
        assert_eq!(remaining, "");
        assert!(values.is_empty());
    }

    #[test]
    fn test_parse_sse_frames_mixed_data_prefixes() {
        let input = "data: {\"a\":1}\nevent: ping\ndata: {\"b\":2}\n\n";
        let (remaining, values) = parse_sse_frames(input);
        assert_eq!(remaining, "");
        assert_eq!(values.len(), 2);
    }

    #[test]
    fn test_parse_sse_frames_multiple_chunks_accumulation() {
        let chunk1 = "data: {\"a\":1}\n\ndata: ";
        let chunk2 = "{\"b\":2}\n\n";
        let (rem1, val1) = parse_sse_frames(chunk1);
        assert_eq!(rem1, "data: ");
        assert_eq!(val1.len(), 1);
        let combined = rem1 + chunk2;
        let (rem2, val2) = parse_sse_frames(&combined);
        assert_eq!(rem2, "");
        assert_eq!(val2.len(), 1);
    }

    #[tokio::test]
    async fn test_to_anthropic_format_passes_through_anthropic_events() {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        let _ = tx.send(serde_json::json!({"type": "message_start", "message": {}}));
        let _ = tx.send(serde_json::json!({"type": "content_block_delta", "index": 0, "delta": {"type": "text_delta", "text": "hi"}}));
        let _ = tx.send(serde_json::json!({"type": "message_stop"}));
        drop(tx);

        let mut stream = to_anthropic_format(rx).rx;
        let mut count = 0;
        while let Some(_) = stream.recv().await {
            count += 1;
        }
        assert_eq!(count, 3);
    }

    #[tokio::test]
    async fn test_to_anthropic_format_converts_openai_chunks() {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        let _ = tx.send(serde_json::json!({
            "id": "chatcmpl-123",
            "model": "gpt-4",
            "choices": [{"index": 0, "delta": {"content": "Hello"}, "finish_reason": null}]
        }));
        let _ = tx.send(serde_json::json!({
            "id": "chatcmpl-123",
            "model": "gpt-4",
            "choices": [{"index": 0, "delta": {"content": " world"}, "finish_reason": null}]
        }));
        let _ = tx.send(serde_json::json!({
            "id": "chatcmpl-123",
            "model": "gpt-4",
            "choices": [{"index": 0, "delta": {}, "finish_reason": "stop"}]
        }));
        drop(tx);

        let mut stream = to_anthropic_format(rx).rx;
        let mut events = Vec::new();
        while let Some(value) = stream.recv().await {
            if let Some(event_type) = value.get("type").and_then(|v| v.as_str()) {
                events.push(event_type.to_string());
            }
        }

        assert!(events.contains(&"message_start".to_string()));
        assert!(events.contains(&"content_block_start".to_string()));
        assert!(events.contains(&"content_block_delta".to_string()));
        assert!(events.contains(&"content_block_stop".to_string()));
        assert!(events.contains(&"message_delta".to_string()));
        assert!(events.contains(&"message_stop".to_string()));
    }
}
