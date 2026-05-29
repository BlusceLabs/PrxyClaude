use std::collections::HashMap;
use std::sync::LazyLock;

/// Status message prefixes used to filter our own messages (ignore echo).
pub const STATUS_MESSAGE_PREFIXES: &[&str] = &[
    "\u{1f3f3}", // ⏳
    "\u{1f4ad}", // 💭
    "\u{1f527}", // 🔧
    "\u{2705}",  // ✅
    "\u{274c}",  // ❌
    "\u{1f680}", // 🚀
    "\u{1f916}", // 🤖
    "\u{1f4cb}", // 📋
    "\u{1f4ca}", // 📊
    "\u{1f504}", // 🔄
];

/// Event types that update the transcript.
pub const TRANSCRIPT_EVENT_TYPES: &[&str] = &[
    "thinking_start",
    "thinking_delta",
    "thinking_chunk",
    "thinking_stop",
    "text_start",
    "text_delta",
    "text_chunk",
    "text_stop",
    "tool_use_start",
    "tool_use_delta",
    "tool_use_stop",
    "tool_use",
    "tool_result",
    "block_stop",
    "error",
];

/// Event type -> (emoji, label) for status updates.
pub static EVENT_STATUS_MAP: LazyLock<HashMap<&'static str, (&'static str, &'static str)>> =
    LazyLock::new(|| {
        let mut m = HashMap::new();
        m.insert("thinking_start", ("\u{1f9e0}", "Claude is thinking..."));
        m.insert("thinking_delta", ("\u{1f9e0}", "Claude is thinking..."));
        m.insert("thinking_chunk", ("\u{1f9e0}", "Claude is thinking..."));
        m.insert("text_start", ("\u{1f9e0}", "Claude is working..."));
        m.insert("text_delta", ("\u{1f9e0}", "Claude is working..."));
        m.insert("text_chunk", ("\u{1f9e0}", "Claude is working..."));
        m.insert("tool_result", ("\u{23f3}", "Executing tools..."));
        m
    });

/// Check if a string starts with any of the status message prefixes.
pub fn is_status_message(text: &str) -> bool {
    STATUS_MESSAGE_PREFIXES.iter().any(|p| text.starts_with(p))
}

/// Return status string for event type, or None if no status update needed.
pub fn get_status_for_event(
    ptype: &str,
    parsed: &serde_json::Value,
    format_status_fn: impl Fn(&str, &str) -> String,
) -> Option<String> {
    if let Some(&(emoji, label)) = EVENT_STATUS_MAP.get(ptype) {
        return Some(format_status_fn(emoji, label));
    }
    if ptype == "tool_use_start" || ptype == "tool_use_delta" || ptype == "tool_use" {
        if parsed.get("name").and_then(|v| v.as_str()) == Some("Task") {
            return Some(format_status_fn("\u{1f916}", "Subagent working..."));
        }
        return Some(format_status_fn("\u{23f3}", "Executing tools..."));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_is_status_message() {
        assert!(is_status_message("\u{1f3f3} Processing..."));
        assert!(is_status_message("\u{274c} Error"));
        assert!(!is_status_message("Hello"));
        assert!(!is_status_message(""));
    }

    #[test]
    fn test_get_status_for_event() {
        let fmt = |e: &str, l: &str| format!("{e} {l}");
        let s = get_status_for_event("thinking_start", &json!({}), fmt);
        assert!(s.is_some());
        assert!(s.unwrap().contains("thinking"));

        let s = get_status_for_event("tool_use_start", &json!({"name": "Task"}), fmt);
        assert!(s.unwrap().contains("Subagent"));

        let s = get_status_for_event("tool_use_start", &json!({"name": "Bash"}), fmt);
        assert!(s.unwrap().contains("Executing"));

        assert!(get_status_for_event("block_stop", &json!({}), fmt).is_none());
    }
}
