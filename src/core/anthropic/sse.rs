use std::collections::HashMap;
use futures::StreamExt;
use serde_json::Value;
use tokio_stream::wrappers::UnboundedReceiverStream;

use crate::providers::ProviderStream;

pub const ANTHROPIC_SSE_RESPONSE_HEADERS: [(&str, &str); 3] = [
    ("X-Accel-Buffering", "no"),
    ("Cache-Control", "no-cache"),
    ("Connection", "keep-alive"),
];

pub fn format_sse_event(event_type: &str, data: &Value) -> String {
    format!("event: {}\ndata: {}\n\n", event_type, data)
}

pub fn map_stop_reason(openai_reason: Option<&str>) -> &'static str {
    match openai_reason {
        Some("stop") => "end_turn",
        Some("length") => "max_tokens",
        Some("tool_calls") => "tool_use",
        Some("content_filter") => "end_turn",
        _ => "end_turn",
    }
}

#[derive(Debug, Clone, Default)]
pub struct ToolCallState {
    pub block_index: i32,
    pub tool_id: String,
    pub name: String,
    pub contents: Vec<String>,
    pub started: bool,
    pub task_arg_buffer: String,
    pub task_args_emitted: bool,
    pub pre_start_args: String,
}

#[derive(Debug, Clone, Default)]
pub struct ContentBlockManager {
    pub next_index: i32,
    pub thinking_index: i32,
    pub text_index: i32,
    pub thinking_started: bool,
    pub text_started: bool,
    pub tool_states: HashMap<i32, ToolCallState>,
}

impl ContentBlockManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn allocate_index(&mut self) -> i32 {
        let idx = self.next_index;
        self.next_index += 1;
        idx
    }

    pub fn ensure_tool_state(&mut self, index: i32) -> &mut ToolCallState {
        self.tool_states
            .entry(index)
            .or_insert_with(|| ToolCallState {
                block_index: -1,
                tool_id: String::new(),
                name: String::new(),
                ..Default::default()
            })
    }

    pub fn set_stream_tool_id(&mut self, index: i32, tool_id: &str) {
        if !tool_id.is_empty() {
            let state = self.ensure_tool_state(index);
            state.tool_id = tool_id.to_string();
        }
    }

    pub fn register_tool_name(&mut self, index: i32, name: &str) {
        let state = self.ensure_tool_state(index);
        if state.name.is_empty() || name.starts_with(&state.name) {
            state.name = name.to_string();
        } else if !state.name.starts_with(name) {
            state.name = format!("{}{}", state.name, name);
        }
    }

    pub fn has_emitted_tool_block(&self) -> bool {
        self.tool_states.values().any(|s| s.started)
    }

    pub fn flush_task_arg_buffers(&mut self) -> Vec<(i32, String)> {
        let mut results = Vec::new();
        for (tool_index, state) in &mut self.tool_states {
            if state.task_arg_buffer.is_empty() || state.task_args_emitted {
                continue;
            }
            let args = normalize_task_run_in_background(&state.task_arg_buffer);
            state.task_args_emitted = true;
            state.task_arg_buffer.clear();
            results.push((*tool_index, args));
        }
        results
    }
}

fn normalize_task_run_in_background(args_json: &str) -> String {
    match serde_json::from_str::<Value>(args_json) {
        Ok(mut val) => {
            if let Some(obj) = val.as_object_mut() {
                if obj.get("run_in_background").and_then(|v| v.as_bool()) != Some(false) {
                    obj.insert("run_in_background".to_string(), Value::Bool(false));
                }
            }
            serde_json::to_string(&val).unwrap_or_else(|_| args_json.to_string())
        }
        Err(_) => args_json.to_string(),
    }
}

#[derive(Debug)]
pub struct SSEBuilder {
    pub message_id: String,
    pub model: String,
    input_tokens: i32,
    log_raw_events: bool,
    pub blocks: ContentBlockManager,
    accumulated_text_parts: Vec<String>,
    accumulated_reasoning_parts: Vec<String>,
}

impl SSEBuilder {
    pub fn new(
        message_id: String,
        model: String,
        input_tokens: i32,
        log_raw_events: bool,
    ) -> Self {
        Self {
            message_id,
            model,
            input_tokens,
            log_raw_events,
            blocks: ContentBlockManager::new(),
            accumulated_text_parts: Vec::new(),
            accumulated_reasoning_parts: Vec::new(),
        }
    }

    fn format_event(&self, event_type: &str, data: &Value) -> String {
        let event_str = format_sse_event(event_type, data);
        if self.log_raw_events {
            tracing::debug!("SSE_EVENT: {} - {}", event_type, event_str.trim());
        } else {
            tracing::debug!(
                "SSE_EVENT: event_type={} serialized_bytes={}",
                event_type,
                event_str.len()
            );
        }
        event_str
    }

    pub fn message_start(&self) -> String {
        let safe_input = self.input_tokens.max(0);
        let data = serde_json::json!({
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
                    "input_tokens": safe_input,
                    "output_tokens": 1,
                },
            },
        });
        self.format_event("message_start", &data)
    }

    pub fn message_delta(&self, stop_reason: &str, output_tokens: Option<i32>) -> String {
        let safe_in = self.input_tokens.max(0);
        let safe_out = output_tokens.unwrap_or(0).max(0);
        let data = serde_json::json!({
            "type": "message_delta",
            "delta": {
                "stop_reason": stop_reason,
                "stop_sequence": null,
            },
            "usage": {
                "input_tokens": safe_in,
                "output_tokens": safe_out,
            },
        });
        self.format_event("message_delta", &data)
    }

    pub fn message_stop(&self) -> String {
        let data = serde_json::json!({"type": "message_stop"});
        self.format_event("message_stop", &data)
    }

    pub fn content_block_start(
        &self,
        index: i32,
        block_type: &str,
        extra: Option<&Value>,
    ) -> String {
        let mut content_block = serde_json::json!({"type": block_type});
        if let Some(extras) = extra {
            if let (Some(cb_obj), Some(extra_obj)) =
                (content_block.as_object_mut(), extras.as_object())
            {
                for (k, v) in extra_obj {
                    cb_obj.insert(k.clone(), v.clone());
                }
            }
        }
        let data = serde_json::json!({
            "type": "content_block_start",
            "index": index,
            "content_block": content_block,
        });
        self.format_event("content_block_start", &data)
    }

    pub fn content_block_delta(&self, index: i32, delta_type: &str, content: &str) -> String {
        let mut delta = serde_json::json!({"type": delta_type});
        match delta_type {
            "thinking_delta" => {
                delta["thinking"] = Value::String(content.to_string());
            }
            "text_delta" => {
                delta["text"] = Value::String(content.to_string());
            }
            "input_json_delta" => {
                delta["partial_json"] = Value::String(content.to_string());
            }
            _ => {}
        }
        let data = serde_json::json!({
            "type": "content_block_delta",
            "index": index,
            "delta": delta,
        });
        self.format_event("content_block_delta", &data)
    }

    pub fn content_block_stop(&self, index: i32) -> String {
        let data = serde_json::json!({
            "type": "content_block_stop",
            "index": index,
        });
        self.format_event("content_block_stop", &data)
    }

    pub fn start_thinking_block(&mut self) -> String {
        let idx = self.blocks.allocate_index();
        self.blocks.thinking_index = idx;
        self.blocks.thinking_started = true;
        self.content_block_start(idx, "thinking", None)
    }

    pub fn emit_thinking_delta(&mut self, content: &str) -> String {
        self.accumulated_reasoning_parts.push(content.to_string());
        self.content_block_delta(self.blocks.thinking_index, "thinking_delta", content)
    }

    pub fn stop_thinking_block(&mut self) -> String {
        self.blocks.thinking_started = false;
        self.content_block_stop(self.blocks.thinking_index)
    }

    pub fn start_text_block(&mut self) -> String {
        let idx = self.blocks.allocate_index();
        self.blocks.text_index = idx;
        self.blocks.text_started = true;
        self.content_block_start(idx, "text", None)
    }

    pub fn emit_text_delta(&mut self, content: &str) -> String {
        self.accumulated_text_parts.push(content.to_string());
        self.content_block_delta(self.blocks.text_index, "text_delta", content)
    }

    pub fn stop_text_block(&mut self) -> String {
        self.blocks.text_started = false;
        self.content_block_stop(self.blocks.text_index)
    }

    pub fn start_tool_block(&mut self, tool_index: i32, tool_id: &str, name: &str) -> String {
        let block_idx = self.blocks.allocate_index();
        let state = self.blocks.tool_states.entry(tool_index).or_insert_with(|| {
            ToolCallState {
                block_index: block_idx,
                tool_id: tool_id.to_string(),
                name: name.to_string(),
                started: true,
                ..Default::default()
            }
        });
        state.block_index = block_idx;
        state.tool_id = tool_id.to_string();
        state.started = true;
        self.content_block_start(
            block_idx,
            "tool_use",
            Some(&serde_json::json!({"id": tool_id, "name": name})),
        )
    }

    pub fn emit_tool_delta(&mut self, tool_index: i32, partial_json: &str) -> String {
        let block_idx = self
            .blocks
            .tool_states
            .get(&tool_index)
            .map(|s| s.block_index)
            .unwrap_or(-1);
        if let Some(state) = self.blocks.tool_states.get_mut(&tool_index) {
            state.contents.push(partial_json.to_string());
        }
        self.content_block_delta(block_idx, "input_json_delta", partial_json)
    }

    pub fn stop_tool_block(&mut self, tool_index: i32) -> String {
        let block_idx = self
            .blocks
            .tool_states
            .get(&tool_index)
            .map(|s| s.block_index)
            .unwrap_or(-1);
        self.content_block_stop(block_idx)
    }

    pub fn ensure_thinking_block(&mut self) -> Vec<String> {
        let mut events = Vec::new();
        if self.blocks.text_started {
            events.push(self.stop_text_block());
        }
        if !self.blocks.thinking_started {
            events.push(self.start_thinking_block());
        }
        events
    }

    pub fn ensure_text_block(&mut self) -> Vec<String> {
        let mut events = Vec::new();
        if self.blocks.thinking_started {
            events.push(self.stop_thinking_block());
        }
        if !self.blocks.text_started {
            events.push(self.start_text_block());
        }
        events
    }

    pub fn close_content_blocks(&mut self) -> Vec<String> {
        let mut events = Vec::new();
        if self.blocks.thinking_started {
            events.push(self.stop_thinking_block());
        }
        if self.blocks.text_started {
            events.push(self.stop_text_block());
        }
        events
    }

    pub fn close_all_blocks(&mut self) -> Vec<String> {
        let mut events = self.close_content_blocks();
        let tool_indices: Vec<i32> = self.blocks.tool_states.keys().copied().collect();
        for tool_index in tool_indices {
            if let Some(state) = self.blocks.tool_states.get(&tool_index) {
                if state.started {
                    events.push(self.stop_tool_block(tool_index));
                }
            }
        }
        events
    }

    pub fn emit_error(&mut self, error_message: &str) -> Vec<String> {
        let error_index = self.blocks.allocate_index();
        vec![
            self.content_block_start(error_index, "text", None),
            self.content_block_delta(error_index, "text_delta", error_message),
            self.content_block_stop(error_index),
        ]
    }

    pub fn emit_top_level_error(&self, error_message: &str) -> String {
        let data = serde_json::json!({
            "type": "error",
            "error": {
                "type": "api_error",
                "message": error_message,
            },
        });
        self.format_event("error", &data)
    }

    pub fn accumulated_text(&self) -> String {
        self.accumulated_text_parts.join("")
    }

    pub fn accumulated_reasoning(&self) -> String {
        self.accumulated_reasoning_parts.join("")
    }

    pub fn estimate_output_tokens(&self) -> i32 {
        let text_tokens = self.accumulated_text().len() as i32 / 4;
        let reasoning_tokens = self.accumulated_reasoning().len() as i32 / 4;
        let tool_tokens: i32 = self
            .blocks
            .tool_states
            .values()
            .filter(|s| s.started)
            .map(|_| 50)
            .sum();
        text_tokens + reasoning_tokens + tool_tokens
    }
}

pub fn map_stop_reason_str(stop_reason: &str) -> Option<&'static str> {
    match stop_reason {
        "end_turn" => Some("end_turn"),
        "max_tokens" => Some("max_tokens"),
        "stop_sequence" => Some("stop_sequence"),
        "tool_use" => Some("tool_use"),
        _ => None,
    }
}

fn estimate_tokens(text: &str) -> i32 {
    (text.len() as f64 / 4.0).ceil() as i32
}

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

        if value.get("error").is_some() && value.get("type").is_none() {
            return self.send(serde_json::json!({
                "type": "error",
                "error": value.get("error"),
            }));
        }

        if value.get("type").and_then(|v| v.as_str()).is_some() {
            return self.send(value);
        }

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
        assert_eq!(map_stop_reason(Some("stop")), "end_turn");
        assert_eq!(map_stop_reason(Some("length")), "max_tokens");
        assert_eq!(map_stop_reason(Some("tool_calls")), "tool_use");
        assert_eq!(map_stop_reason(None), "end_turn");
    }

    #[test]
    fn test_estimate_tokens() {
        assert_eq!(estimate_tokens("hello"), 2);
        assert_eq!(estimate_tokens(""), 0);
    }

    #[test]
    fn test_content_block_manager() {
        let mut mgr = ContentBlockManager::new();
        assert_eq!(mgr.allocate_index(), 0);
        assert_eq!(mgr.allocate_index(), 1);
        mgr.thinking_started = true;
        mgr.thinking_index = 0;
        assert!(mgr.thinking_started);
    }

    #[test]
    fn test_tool_call_state() {
        let mut mgr = ContentBlockManager::new();
        mgr.set_stream_tool_id(0, "tool_123");
        assert_eq!(mgr.tool_states[&0].tool_id, "tool_123");

        mgr.register_tool_name(0, "my_");
        assert_eq!(mgr.tool_states[&0].name, "my_");
        mgr.register_tool_name(0, "tool");
        assert_eq!(mgr.tool_states[&0].name, "my_tool");
    }

    #[test]
    fn test_sse_builder_lifecycle() {
        let mut sse = SSEBuilder::new("msg_test".to_string(), "gpt-4".to_string(), 10, false);

        let start = sse.message_start();
        assert!(start.contains("message_start"));

        let events = sse.ensure_text_block();
        assert!(!events.is_empty());

        let delta = sse.emit_text_delta("hello");
        assert!(delta.contains("text_delta"));

        let events = sse.close_all_blocks();
        assert!(!events.is_empty());

        let stop = sse.message_stop();
        assert!(stop.contains("message_stop"));
    }

    #[test]
    fn test_normalize_task_run_in_background() {
        let input = r#"{"run_in_background": true, "task": "test"}"#;
        let output = normalize_task_run_in_background(input);
        let parsed: Value = serde_json::from_str(&output).unwrap();
        assert_eq!(parsed["run_in_background"], false);
    }

    #[test]
    fn test_parse_sse_frames() {
        let input = "data: {\"key\": \"value\"}\n\n";
        let (remaining, values) = parse_sse_frames(input);
        assert_eq!(remaining, "");
        assert_eq!(values.len(), 1);
    }

    #[test]
    fn test_parse_sse_frames_done() {
        let input = "data: {\"a\":1}\n\ndata: [DONE]\n\ndata: {\"b\":2}\n\n";
        let (remaining, values) = parse_sse_frames(input);
        assert_eq!(remaining, "");
        assert_eq!(values.len(), 2);
    }

    #[tokio::test]
    async fn test_to_anthropic_format() {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        let _ = tx.send(serde_json::json!({"type": "message_start", "message": {}}));
        let _ = tx.send(serde_json::json!({"type": "message_stop"}));
        drop(tx);

        let mut stream = to_anthropic_format(rx).rx;
        let mut count = 0;
        while let Some(_) = stream.recv().await {
            count += 1;
        }
        assert_eq!(count, 2);
    }
}
