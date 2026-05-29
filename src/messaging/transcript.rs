use std::collections::{HashMap, VecDeque};

use crate::messaging::rendering::profiles::RenderCtx;

/// A segment in the transcript.
#[derive(Debug, Clone)]
pub enum Segment {
    Thinking(ThinkingSegment),
    Text(TextSegment),
    ToolCall(ToolCallSegment),
    ToolResult(ToolResultSegment),
    Subagent(SubagentSegment),
    Error(ErrorSegment),
}

#[derive(Debug, Clone, Default)]
pub struct ThinkingSegment {
    parts: Vec<String>,
}

impl ThinkingSegment {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn append(&mut self, t: &str) {
        if !t.is_empty() {
            self.parts.push(t.to_string());
        }
    }

    pub fn text(&self) -> String {
        self.parts.join("")
    }

    pub fn render(&self, ctx: &RenderCtx) -> String {
        let mut raw = self.text();
        if let Some(max) = ctx.thinking_tail_max {
            if raw.len() > max {
                raw = format!("...{}", &raw[raw.len() - (max - 3)..]);
            }
        }
        let inner = (ctx.escape_code)(&raw);
        format!("\u{1f4ad} {} \n```\n{inner}\n```", (ctx.bold)("Thinking"))
    }
}

#[derive(Debug, Clone, Default)]
pub struct TextSegment {
    parts: Vec<String>,
}

impl TextSegment {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn append(&mut self, t: &str) {
        if !t.is_empty() {
            self.parts.push(t.to_string());
        }
    }

    pub fn text(&self) -> String {
        self.parts.join("")
    }

    pub fn render(&self, ctx: &RenderCtx) -> String {
        let mut raw = self.text();
        if let Some(max) = ctx.text_tail_max {
            if raw.len() > max {
                raw = format!("...{}", &raw[raw.len() - (max - 3)..]);
            }
        }
        (ctx.render_markdown)(&raw)
    }
}

#[derive(Debug, Clone)]
pub struct ToolCallSegment {
    pub tool_use_id: String,
    pub name: String,
    pub closed: bool,
    pub indent_level: usize,
}

impl ToolCallSegment {
    pub fn new(tool_use_id: &str, name: &str, indent_level: usize) -> Self {
        Self {
            tool_use_id: tool_use_id.to_string(),
            name: name.to_string(),
            closed: false,
            indent_level,
        }
    }

    pub fn render(&self, ctx: &RenderCtx) -> String {
        let name = (ctx.code_inline)(&self.name);
        let prefix = "  ".repeat(self.indent_level);
        format!("{prefix}\u{1f6e0} {} {name}", (ctx.bold)("Tool call:"))
    }
}

#[derive(Debug, Clone)]
pub struct ToolResultSegment {
    pub tool_use_id: String,
    pub name: Option<String>,
    pub content_text: String,
    pub is_error: bool,
}

impl ToolResultSegment {
    pub fn new(tool_use_id: &str, content: &str, name: Option<&str>, is_error: bool) -> Self {
        Self {
            tool_use_id: tool_use_id.to_string(),
            content_text: content.to_string(),
            name: name.map(String::from),
            is_error,
        }
    }

    pub fn render(&self, ctx: &RenderCtx) -> String {
        let mut raw = self.content_text.clone();
        if let Some(max) = ctx.tool_output_tail_max {
            if raw.len() > max {
                raw = format!("...{}", &raw[raw.len() - (max - 3)..]);
            }
        }
        let inner = (ctx.escape_code)(&raw);
        let label = if self.is_error {
            "Tool error:"
        } else {
            "Tool result:"
        };
        let maybe_name = self
            .name
            .as_ref()
            .map(|n| format!(" {}", (ctx.code_inline)(n)))
            .unwrap_or_default();
        format!(
            "\u{1f4e4} {}{maybe_name}\n```\n{inner}\n```",
            (ctx.bold)(label)
        )
    }
}

#[derive(Debug, Clone)]
pub struct SubagentSegment {
    pub description: String,
    pub tool_calls: usize,
    pub tools_used: std::collections::HashSet<String>,
    pub current_tool: Option<ToolCallSegment>,
}

impl SubagentSegment {
    pub fn new(description: &str) -> Self {
        Self {
            description: description.to_string(),
            tool_calls: 0,
            tools_used: std::collections::HashSet::new(),
            current_tool: None,
        }
    }

    pub fn set_current_tool_call(&mut self, tool_use_id: &str, name: &str) -> ToolCallSegment {
        self.tools_used.insert(name.to_string());
        self.tool_calls += 1;
        let seg = ToolCallSegment::new(tool_use_id, name, 1);
        self.current_tool = Some(seg.clone());
        seg
    }

    pub fn render(&self, ctx: &RenderCtx) -> String {
        let mut lines = vec![format!(
            "\u{1f916} {} {}",
            (ctx.bold)("Subagent:"),
            (ctx.code_inline)(&self.description)
        )];

        if let Some(tool) = &self.current_tool {
            let rendered = tool.render(ctx);
            if !rendered.is_empty() {
                lines.push(rendered);
            }
        }

        let mut tools_used: Vec<&String> = self.tools_used.iter().collect();
        tools_used.sort();
        let tools_set_raw = if tools_used.is_empty() {
            "{}".to_string()
        } else {
            let joined: Vec<&str> = tools_used.iter().map(|s| s.as_str()).collect();
            format!("{{{}}}", joined.join(", "))
        };

        lines.push(format!(
            "  {} {}",
            (ctx.bold)("Tools used:"),
            (ctx.code_inline)(&tools_set_raw)
        ));
        lines.push(format!(
            "  {} {}",
            (ctx.bold)("Tool calls:"),
            (ctx.code_inline)(&self.tool_calls.to_string())
        ));
        lines.join("\n")
    }
}

#[derive(Debug, Clone)]
pub struct ErrorSegment {
    pub message: String,
}

impl ErrorSegment {
    pub fn new(message: &str) -> Self {
        Self {
            message: message.to_string(),
        }
    }

    pub fn render(&self, ctx: &RenderCtx) -> String {
        format!(
            "\u{26a0}\u{fe0f} {} {}",
            (ctx.bold)("Error:"),
            (ctx.code_inline)(&self.message)
        )
    }
}

/// Maintains an ordered, truncatable transcript of events.
pub struct TranscriptBuffer {
    segments: Vec<Segment>,
    open_thinking_by_index: HashMap<i64, usize>,
    open_text_by_index: HashMap<i64, usize>,
    open_tools_by_index: HashMap<i64, usize>,
    tool_name_by_id: HashMap<String, String>,
    show_tool_results: bool,
    subagent_stack: Vec<String>,
    subagent_segments: Vec<usize>,
}

impl TranscriptBuffer {
    pub fn new(show_tool_results: bool) -> Self {
        Self {
            segments: Vec::new(),
            open_thinking_by_index: HashMap::new(),
            open_text_by_index: HashMap::new(),
            open_tools_by_index: HashMap::new(),
            tool_name_by_id: HashMap::new(),
            show_tool_results,
            subagent_stack: Vec::new(),
            subagent_segments: Vec::new(),
        }
    }

    fn in_subagent(&self) -> bool {
        !self.subagent_stack.is_empty()
    }

    fn subagent_current(&self) -> Option<usize> {
        self.subagent_segments.last().copied()
    }

    fn subagent_push(&mut self, tool_id: &str, seg_idx: usize) {
        let id = if tool_id.is_empty() {
            format!("__task_{}", self.subagent_stack.len() + 1)
        } else {
            tool_id.to_string()
        };
        self.subagent_stack.push(id);
        self.subagent_segments.push(seg_idx);
    }

    fn subagent_pop(&mut self, tool_id: &str) -> bool {
        if self.subagent_stack.is_empty() {
            return false;
        }

        let tool_id = tool_id.to_string();
        if !tool_id.is_empty() {
            // LIFO check
            if let Some(top) = self.subagent_stack.last() {
                if top == &tool_id
                    || top.starts_with(&tool_id)
                    || tool_id.starts_with(top.as_str())
                {
                    self.subagent_stack.pop();
                    self.subagent_segments.pop();
                    return true;
                }
            }
            // Search from end
            if let Some(idx) = self.subagent_stack.iter().rposition(|id| {
                id == &tool_id || id.starts_with(&tool_id) || tool_id.starts_with(id.as_str())
            }) {
                while self.subagent_stack.len() > idx {
                    self.subagent_stack.pop();
                    self.subagent_segments.pop();
                }
                return true;
            }
            return false;
        }

        // No id; only close synthetic
        if let Some(top) = self.subagent_stack.last() {
            if top.starts_with("__task_") {
                self.subagent_stack.pop();
                self.subagent_segments.pop();
                return true;
            }
        }
        false
    }

    /// Apply a parsed event to the transcript.
    pub fn apply(&mut self, ev: &serde_json::Value) {
        let et = ev.get("type").and_then(|v| v.as_str()).unwrap_or("");

        // Subagent rules
        if self.in_subagent()
            && matches!(
                et,
                "thinking_start" | "thinking_delta" | "thinking_chunk"
                    | "text_start" | "text_delta" | "text_chunk"
            )
        {
            return;
        }

        match et {
            "thinking_start" => {
                let idx = ev.get("index").and_then(|v| v.as_i64()).unwrap_or(-1);
                if idx >= 0 {
                    self.apply(&serde_json::json!({"type": "block_stop", "index": idx}));
                }
                let seg = ThinkingSegment::new();
                self.segments.push(Segment::Thinking(seg));
                let seg_idx = self.segments.len() - 1;
                if idx >= 0 {
                    self.open_thinking_by_index.insert(idx, seg_idx);
                }
            }
            "thinking_delta" | "thinking_chunk" => {
                let idx = ev.get("index").and_then(|v| v.as_i64()).unwrap_or(-1);
                let text = ev.get("text").and_then(|v| v.as_str()).unwrap_or("");
                if let Some(&seg_idx) = self.open_thinking_by_index.get(&idx) {
                    if let Segment::Thinking(seg) = &mut self.segments[seg_idx] {
                        seg.append(text);
                    }
                } else {
                    let mut seg = ThinkingSegment::new();
                    seg.append(text);
                    self.segments.push(Segment::Thinking(seg));
                    let new_idx = self.segments.len() - 1;
                    if idx >= 0 {
                        self.open_thinking_by_index.insert(idx, new_idx);
                    }
                }
            }
            "thinking_stop" => {
                let idx = ev.get("index").and_then(|v| v.as_i64()).unwrap_or(-1);
                if idx >= 0 {
                    self.open_thinking_by_index.remove(&idx);
                }
            }
            "text_start" => {
                let idx = ev.get("index").and_then(|v| v.as_i64()).unwrap_or(-1);
                if idx >= 0 {
                    self.apply(&serde_json::json!({"type": "block_stop", "index": idx}));
                }
                let seg = TextSegment::new();
                self.segments.push(Segment::Text(seg));
                let seg_idx = self.segments.len() - 1;
                if idx >= 0 {
                    self.open_text_by_index.insert(idx, seg_idx);
                }
            }
            "text_delta" | "text_chunk" => {
                let idx = ev.get("index").and_then(|v| v.as_i64()).unwrap_or(-1);
                let text = ev.get("text").and_then(|v| v.as_str()).unwrap_or("");
                if let Some(&seg_idx) = self.open_text_by_index.get(&idx) {
                    if let Segment::Text(seg) = &mut self.segments[seg_idx] {
                        seg.append(text);
                    }
                } else {
                    let mut seg = TextSegment::new();
                    seg.append(text);
                    self.segments.push(Segment::Text(seg));
                    let new_idx = self.segments.len() - 1;
                    if idx >= 0 {
                        self.open_text_by_index.insert(idx, new_idx);
                    }
                }
            }
            "text_stop" => {
                let idx = ev.get("index").and_then(|v| v.as_i64()).unwrap_or(-1);
                if idx >= 0 {
                    self.open_text_by_index.remove(&idx);
                }
            }
            "tool_use_start" => {
                let idx = ev.get("index").and_then(|v| v.as_i64()).unwrap_or(-1);
                if idx >= 0 {
                    self.apply(&serde_json::json!({"type": "block_stop", "index": idx}));
                }
                let tool_id = ev
                    .get("id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .trim()
                    .to_string();
                let name = ev
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("tool")
                    .to_string();

                if !tool_id.is_empty() {
                    self.tool_name_by_id.insert(tool_id.clone(), name.clone());
                }

                if name == "Task" {
                    let heading = self.task_heading_from_input(ev.get("input"));
                    let seg = SubagentSegment::new(&heading);
                    self.segments.push(Segment::Subagent(seg));
                    let seg_idx = self.segments.len() - 1;
                    self.subagent_push(&tool_id, seg_idx);
                    return;
                }

                let seg = if self.in_subagent() {
                    if let Some(parent_idx) = self.subagent_current() {
                        if let Segment::Subagent(parent) = &mut self.segments[parent_idx] {
                            parent.set_current_tool_call(&tool_id, &name)
                        } else {
                            ToolCallSegment::new(&tool_id, &name, 0)
                        }
                    } else {
                        ToolCallSegment::new(&tool_id, &name, 0)
                    }
                } else {
                    ToolCallSegment::new(&tool_id, &name, 0)
                };

                self.segments.push(Segment::ToolCall(seg));
                let seg_idx = self.segments.len() - 1;
                if idx >= 0 {
                    self.open_tools_by_index.insert(idx, seg_idx);
                }
            }
            "tool_use_delta" => {}
            "tool_use_stop" => {
                let idx = ev.get("index").and_then(|v| v.as_i64()).unwrap_or(-1);
                if let Some(&seg_idx) = self.open_tools_by_index.get(&idx) {
                    if let Segment::ToolCall(seg) = &mut self.segments[seg_idx] {
                        seg.closed = true;
                    }
                    self.open_tools_by_index.remove(&idx);
                }
            }
            "block_stop" => {
                let idx = ev.get("index").and_then(|v| v.as_i64()).unwrap_or(-1);
                if self.open_tools_by_index.contains_key(&idx) {
                    self.apply(&serde_json::json!({"type": "tool_use_stop", "index": idx}));
                } else if self.open_thinking_by_index.contains_key(&idx) {
                    self.apply(&serde_json::json!({"type": "thinking_stop", "index": idx}));
                } else if self.open_text_by_index.contains_key(&idx) {
                    self.apply(&serde_json::json!({"type": "text_stop", "index": idx}));
                }
            }
            "tool_use" => {
                let tool_id = ev
                    .get("id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .trim()
                    .to_string();
                let name = ev
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("tool")
                    .to_string();

                if !tool_id.is_empty() {
                    self.tool_name_by_id.insert(tool_id.clone(), name.clone());
                }

                if name == "Task" {
                    let heading = self.task_heading_from_input(ev.get("input"));
                    let seg = SubagentSegment::new(&heading);
                    self.segments.push(Segment::Subagent(seg));
                    let seg_idx = self.segments.len() - 1;
                    self.subagent_push(&tool_id, seg_idx);
                    return;
                }

                let seg = if self.in_subagent() {
                    if let Some(parent_idx) = self.subagent_current() {
                        if let Segment::Subagent(parent) = &mut self.segments[parent_idx] {
                            parent.set_current_tool_call(&tool_id, &name)
                        } else {
                            ToolCallSegment::new(&tool_id, &name, 0)
                        }
                    } else {
                        ToolCallSegment::new(&tool_id, &name, 0)
                    }
                } else {
                    ToolCallSegment::new(&tool_id, &name, 0)
                };

                self.segments.push(Segment::ToolCall(seg));
                if let Some(Segment::ToolCall(seg)) = self.segments.last_mut() {
                    seg.closed = true;
                }
            }
            "tool_result" => {
                let tool_id = ev
                    .get("tool_use_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .trim()
                    .to_string();
                let name = self.tool_name_by_id.get(&tool_id).cloned();

                // Check subagent context
                if !self.subagent_stack.is_empty() {
                    self.subagent_pop(&tool_id);
                }

                if !self.show_tool_results {
                    return;
                }

                let content = ev
                    .get("content")
                    .map(|c| {
                        if c.is_string() {
                            c.as_str().unwrap_or("").to_string()
                        } else {
                            serde_json::to_string_pretty(c).unwrap_or_default()
                        }
                    })
                    .unwrap_or_default();
                let is_error = ev
                    .get("is_error")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);

                let seg = ToolResultSegment::new(&tool_id, &content, name.as_deref(), is_error);
                self.segments.push(Segment::ToolResult(seg));
            }
            "error" => {
                let message = ev
                    .get("message")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Unknown error");
                self.segments.push(Segment::Error(ErrorSegment::new(message)));
            }
            _ => {}
        }
    }

    fn task_heading_from_input(&self, input: Option<&serde_json::Value>) -> String {
        if let Some(obj) = input.and_then(|v| v.as_object()) {
            if let Some(desc) = obj.get("description").and_then(|v| v.as_str()) {
                let desc = desc.trim();
                if !desc.is_empty() {
                    return desc.to_string();
                }
            }
            if let Some(t) = obj.get("subagent_type").and_then(|v| v.as_str()) {
                let t = t.trim();
                if !t.is_empty() {
                    return t.to_string();
                }
            }
        }
        "Subagent".to_string()
    }

    /// Render transcript with truncation (drop oldest segments).
    pub fn render(&self, ctx: &RenderCtx, limit_chars: usize, status: Option<&str>) -> String {
        let rendered: Vec<String> = self
            .segments
            .iter()
            .filter_map(|seg| {
                let out = match seg {
                    Segment::Thinking(s) => s.render(ctx),
                    Segment::Text(s) => s.render(ctx),
                    Segment::ToolCall(s) => s.render(ctx),
                    Segment::ToolResult(s) => s.render(ctx),
                    Segment::Subagent(s) => s.render(ctx),
                    Segment::Error(s) => s.render(ctx),
                };
                if out.is_empty() {
                    None
                } else {
                    Some(out)
                }
            })
            .collect();

        let status_text = status
            .map(|s| format!("\n\n{s}"))
            .unwrap_or_default();
        let prefix_marker = (ctx.escape_text)("... (truncated)\n");

        let join = |parts: &[String], add_marker: bool| -> String {
            let body = parts.join("\n");
            let body = if add_marker && !body.is_empty() {
                format!("{prefix_marker}{body}")
            } else {
                body
            };
            if !body.is_empty() || !status_text.is_empty() {
                format!("{body}{status_text}")
            } else {
                status_text.clone()
            }
        };

        // Fast path
        let candidate = join(&rendered, false);
        if candidate.len() <= limit_chars {
            return candidate;
        }

        // Drop oldest segments
        let mut parts: VecDeque<String> = rendered.into();
        let mut dropped = false;
        let mut last_part: Option<String> = None;

        while !parts.is_empty() {
            let candidate = join(&parts.iter().cloned().collect::<Vec<_>>(), true);
            if candidate.len() <= limit_chars {
                return candidate;
            }
            last_part = parts.pop_front();
            dropped = true;
        }

        // Nothing fits - preserve tail of last segment
        if dropped {
            if let Some(last) = last_part {
                let budget = limit_chars - prefix_marker.len() - status_text.len();
                if budget > 20 {
                    let tail = if last.len() > budget {
                        format!("...{}", &last[last.len() - (budget - 3)..])
                    } else {
                        last
                    };
                    let candidate = format!("{prefix_marker}{tail}{status_text}");
                    if candidate.len() <= limit_chars {
                        return candidate;
                    }
                }
            }
        }

        // Fallback
        if dropped {
            let minimal = format!("{prefix_marker}{}", status_text.trim_start_matches('\n'));
            if minimal.len() <= limit_chars {
                return minimal;
            }
        }

        status.unwrap_or("").to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::messaging::rendering::profiles::RenderCtx;

    fn default_ctx() -> RenderCtx {
        RenderCtx::default()
    }

    #[test]
    fn test_thinking_segment() {
        let mut seg = ThinkingSegment::new();
        seg.append("hello");
        seg.append(" world");
        assert_eq!(seg.text(), "hello world");
    }

    #[test]
    fn test_text_segment() {
        let mut seg = TextSegment::new();
        seg.append("hello");
        assert_eq!(seg.text(), "hello");
    }

    #[test]
    fn test_transcript_apply_thinking() {
        let mut buf = TranscriptBuffer::new(false);
        buf.apply(&serde_json::json!({"type": "thinking_start", "index": 0}));
        buf.apply(&serde_json::json!({"type": "thinking_delta", "index": 0, "text": "hello"}));
        buf.apply(&serde_json::json!({"type": "thinking_stop", "index": 0}));
        assert_eq!(buf.segments.len(), 1);
        let ctx = default_ctx();
        let rendered = buf.render(&ctx, 1000, None);
        assert!(rendered.contains("hello"));
    }

    #[test]
    fn test_transcript_apply_text() {
        let mut buf = TranscriptBuffer::new(false);
        buf.apply(&serde_json::json!({"type": "text_start", "index": 0}));
        buf.apply(&serde_json::json!({"type": "text_delta", "index": 0, "text": "world"}));
        buf.apply(&serde_json::json!({"type": "text_stop", "index": 0}));
        assert_eq!(buf.segments.len(), 1);
    }

    #[test]
    fn test_transcript_error() {
        let mut buf = TranscriptBuffer::new(false);
        buf.apply(&serde_json::json!({"type": "error", "message": "oops"}));
        assert_eq!(buf.segments.len(), 1);
        let ctx = default_ctx();
        let rendered = buf.render(&ctx, 1000, None);
        assert!(rendered.contains("oops"));
    }

    #[test]
    fn test_transcript_render_truncation() {
        let mut buf = TranscriptBuffer::new(false);
        for i in 0..100 {
            buf.apply(&serde_json::json!({"type": "text_start", "index": i}));
            buf.apply(&serde_json::json!({"type": "text_delta", "index": i, "text": &format!("line {i} ")}));
            buf.apply(&serde_json::json!({"type": "text_stop", "index": i}));
        }
        let ctx = default_ctx();
        let rendered = buf.render(&ctx, 200, None);
        assert!(rendered.len() <= 200);
    }
}
