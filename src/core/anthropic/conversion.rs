use serde_json::Value;

use crate::api::models::{
    ContentBlock, ContentOrBlocks, Message, MessagesRequest, MessagesResponse, Role,
    SystemContentOrString, Tool, Usage,
};
use crate::core::anthropic::utils::set_if_not_none;

#[derive(Debug, Clone, thiserror::Error)]
#[error("OpenAI conversion error: {0}")]
pub struct OpenAIConversionError(pub String);

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReasoningReplayMode {
    Disabled,
    ThinkTags,
    ReasoningContent,
}

impl Default for ReasoningReplayMode {
    fn default() -> Self {
        Self::ThinkTags
    }
}

#[derive(Debug, Clone, Default)]
pub struct PendingAfterTools {
    pub remaining_tool_ids: std::collections::HashSet<String>,
    pub deferred_blocks: Vec<ContentBlock>,
    pub top_level_reasoning: Option<String>,
    pub reasoning_replay: ReasoningReplayMode,
    pub deferred_emitted: bool,
}

impl PendingAfterTools {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn needs_deferred(&self) -> bool {
        !self.deferred_blocks.is_empty() && !self.deferred_emitted
    }
}

fn block_type(block: &ContentBlock) -> &'static str {
    match block {
        ContentBlock::Text(_) => "text",
        ContentBlock::Image(_) => "image",
        ContentBlock::Document(_) => "document",
        ContentBlock::ToolUse(_) => "tool_use",
        ContentBlock::ToolResult(_) => "tool_result",
        ContentBlock::Thinking(_) => "thinking",
        ContentBlock::RedactedThinking(_) => "redacted_thinking",
        ContentBlock::ServerToolUse(_) => "server_tool_use",
        ContentBlock::WebSearchToolResult(_) => "web_search_tool_result",
        ContentBlock::WebFetchToolResult(_) => "web_fetch_tool_result",
    }
}

pub fn clean_reasoning_content(value: &Option<String>) -> Option<String> {
    value.as_ref().filter(|s| !s.is_empty()).cloned()
}

pub fn think_tag_content(reasoning: &str) -> String {
    format!("<thinking>\n{}\n</thinking>", reasoning)
}

pub fn serialize_tool_result_content(tool_content: &Value) -> String {
    match tool_content {
        Value::String(s) => s.clone(),
        Value::Object(obj) => serde_json::to_string(obj).unwrap_or_default(),
        Value::Array(arr) => {
            let mut parts = Vec::new();
            for item in arr {
                if item.get("type").and_then(|t| t.as_str()) == Some("text") {
                    if let Some(text) = item.get("text").and_then(|t| t.as_str()) {
                        parts.push(text.to_string());
                    }
                } else {
                    parts.push(serde_json::to_string(item).unwrap_or_default());
                }
            }
            parts.join("\n")
        }
        _ => tool_content.to_string(),
    }
}

fn index_first_tool_use(blocks: &[ContentBlock]) -> Option<usize> {
    blocks.iter().position(|b| matches!(b, ContentBlock::ToolUse(_)))
}

fn iter_tool_uses_in_order(blocks: &[ContentBlock]) -> Vec<Value> {
    let mut tool_calls = Vec::new();
    for block in blocks {
        if let ContentBlock::ToolUse(tool_use) = block {
            let args = serde_json::to_string(&tool_use.input).unwrap_or_default();
            tool_calls.push(serde_json::json!({
                "id": tool_use.id,
                "type": "function",
                "function": {
                    "name": tool_use.name,
                    "arguments": args,
                },
            }));
        }
    }
    tool_calls
}

fn deferred_post_tool_blocks(content: &[ContentBlock], first_tool_index: usize) -> Vec<ContentBlock> {
    content
        .iter()
        .enumerate()
        .filter(|(i, b)| *i > first_tool_index && !matches!(b, ContentBlock::ToolUse(_)))
        .map(|(_, b)| b.clone())
        .collect()
}

fn assert_no_forbidden_assistant_block(block: &ContentBlock) -> Result<(), OpenAIConversionError> {
    match block {
        ContentBlock::Image(_) => Err(OpenAIConversionError(
            "Assistant image blocks are not supported for OpenAI chat conversion.".to_string(),
        )),
        ContentBlock::ServerToolUse(_) | ContentBlock::WebSearchToolResult(_) | ContentBlock::WebFetchToolResult(_) => {
            Err(OpenAIConversionError(format!(
                "OpenAI chat conversion does not support Anthropic server tool blocks ('{}' in an assistant message). Use a native Anthropic transport provider.",
                block_type(block)
            )))
        }
        _ => Ok(()),
    }
}

fn block_to_json(block: &ContentBlock) -> Value {
    match block {
        ContentBlock::Text(t) => serde_json::json!({
            "type": "text",
            "text": t.text,
        }),
        ContentBlock::Image(img) => serde_json::json!({
            "type": "image",
            "source": img.source,
        }),
        ContentBlock::Document(doc) => serde_json::json!({
            "type": "document",
            "source": doc.source,
        }),
        ContentBlock::ToolUse(tool_use) => serde_json::json!({
            "type": "tool_use",
            "id": tool_use.id,
            "name": tool_use.name,
            "input": tool_use.input,
        }),
        ContentBlock::ToolResult(tr) => serde_json::json!({
            "type": "tool_result",
            "tool_use_id": tr.tool_use_id,
            "content": tr.content,
        }),
        ContentBlock::Thinking(th) => serde_json::json!({
            "type": "thinking",
            "thinking": th.thinking,
            "signature": th.signature,
        }),
        ContentBlock::RedactedThinking(rt) => serde_json::json!({
            "type": "redacted_thinking",
            "data": rt.data,
        }),
        ContentBlock::ServerToolUse(stu) => serde_json::json!({
            "type": "server_tool_use",
            "id": stu.id,
            "name": stu.name,
            "input": stu.input,
        }),
        ContentBlock::WebSearchToolResult(wstr) => serde_json::json!({
            "type": "web_search_tool_result",
            "tool_use_id": wstr.tool_use_id,
            "content": wstr.content,
        }),
        ContentBlock::WebFetchToolResult(wftr) => serde_json::json!({
            "type": "web_fetch_tool_result",
            "tool_use_id": wftr.tool_use_id,
            "content": wftr.content,
        }),
    }
}

pub struct AnthropicToOpenAIConverter;

impl AnthropicToOpenAIConverter {
    pub fn convert_messages(
        messages: &[Message],
        reasoning_replay: ReasoningReplayMode,
    ) -> Vec<Value> {
        let mut result: Vec<Value> = Vec::new();
        let mut pending: Option<PendingAfterTools> = None;

        for msg in messages {
            let role = msg.role.to_string();
            let reasoning_content = clean_reasoning_content(&msg.reasoning_content);

            match &msg.content {
                ContentOrBlocks::Blocks(blocks) if role == "assistant" => {
                    if pending.as_ref().map(|p| p.needs_deferred()).unwrap_or(false) {
                        let pending_clone = pending.take().unwrap();
                        result.extend(Self::deferred_post_tool_to_messages(&pending_clone));
                    }

                    if let Some(idx) = index_first_tool_use(blocks) {
                        for block in blocks {
                            if !matches!(block, ContentBlock::ToolUse(_)) {
                                let _ = assert_no_forbidden_assistant_block(block);
                            }
                        }
                        let (out, new_pending) = Self::convert_assistant_message_with_split(
                            blocks,
                            idx,
                            reasoning_content.as_deref(),
                            reasoning_replay.clone(),
                        );
                        result.extend(out);
                        if new_pending.is_some() {
                            pending = new_pending;
                        }
                    } else {
                        for block in blocks {
                            let _ = assert_no_forbidden_assistant_block(block);
                        }
                        result.extend(Self::convert_assistant_message(
                            blocks,
                            reasoning_content.as_deref(),
                            reasoning_replay.clone(),
                        ));
                    }
                }
                ContentOrBlocks::String(s) => {
                    if role == "user" && pending.as_ref().map(|p| p.needs_deferred()).unwrap_or(false) {
                        let pending_clone = pending.take().unwrap();
                        result.extend(Self::deferred_post_tool_to_messages(&pending_clone));
                    }
                    let mut converted = serde_json::json!({
                        "role": role,
                        "content": s
                    });
                    if role == "assistant" {
                        if let Some(rc) = &reasoning_content {
                            if reasoning_replay == ReasoningReplayMode::ReasoningContent {
                                converted["reasoning_content"] = rc.clone().into();
                            } else if reasoning_replay == ReasoningReplayMode::ThinkTags {
                                let mut content_parts = vec![think_tag_content(rc)];
                                if !s.is_empty() {
                                    content_parts.push(s.clone());
                                }
                                converted["content"] = Value::String(content_parts.join("\n\n"));
                            }
                        }
                    }
                    result.push(converted);
                }
                ContentOrBlocks::Blocks(blocks) => {
                    if role == "user" {
                        if pending.as_ref().map(|p| p.needs_deferred()).unwrap_or(false) {
                            let remaining = pending.as_ref().unwrap().remaining_tool_ids.clone();
                            if remaining.is_empty() {
                                let pending_clone = pending.take().unwrap();
                                result.extend(Self::deferred_post_tool_to_messages(&pending_clone));
                                result.extend(Self::convert_user_message(blocks));
                            } else {
                                let pieces = Self::convert_user_message_with_injection(blocks, pending.as_ref().unwrap());
                                if let Some(msgs) = pieces.get("messages").and_then(|v| v.as_array()) {
                                    result.extend(msgs.iter().cloned());
                                }
                                if pieces.get("cleared_pending").and_then(|v| v.as_bool()).unwrap_or(false) {
                                    pending = None;
                                } else {
                                    result.extend(Self::convert_user_message(blocks));
                                }
                            }
                        } else {
                            result.extend(Self::convert_user_message(blocks));
                        }
                    }
                }

            }
        }

        if pending.as_ref().map(|p| p.needs_deferred()).unwrap_or(false) {
            let pending_clone = pending.unwrap();
            result.extend(Self::deferred_post_tool_to_messages(&pending_clone));
        }

        result
    }

    pub fn convert_assistant_message_with_split(
        content: &[ContentBlock],
        first_tool_index: usize,
        reasoning_content: Option<&str>,
        reasoning_replay: ReasoningReplayMode,
    ) -> (Vec<Value>, Option<PendingAfterTools>) {
        let pre: Vec<ContentBlock> = content[..first_tool_index].to_vec();
        let tool_calls = iter_tool_uses_in_order(content);

        if tool_calls.is_empty() {
            return (
                Self::convert_assistant_message(content, reasoning_content, reasoning_replay),
                None,
            );
        }

        let deferred_blocks = deferred_post_tool_blocks(content, first_tool_index);

        let pre_msg = if pre.is_empty() {
            let mut msg = serde_json::json!({
                "role": "assistant",
                "content": "",
            });
            if reasoning_replay == ReasoningReplayMode::ReasoningContent {
                if let Some(rc) = reasoning_content {
                    msg["reasoning_content"] = Value::String(rc.to_string());
                }
            }
            msg
        } else {
            let converted = Self::convert_assistant_message(
                &pre,
                reasoning_content,
                reasoning_replay.clone(),
            );
            let mut msg = converted.into_iter().next().unwrap_or(serde_json::json!({}));
            msg["tool_calls"] = serde_json::json!(tool_calls);
            msg
        };

        let mut pre_msg = pre_msg;
        if pre_msg.get("content").and_then(|c| c.as_str()) == Some(" ") {
            if let Some(obj) = pre_msg.as_object_mut() {
                obj.insert("content".to_string(), Value::String(String::new()));
            }
        }

        let pnd = if !deferred_blocks.is_empty() {
            let res_ids: std::collections::HashSet<String> = tool_calls
                .iter()
                .filter_map(|tc| tc.get("id").and_then(|v| v.as_str().map(|s| s.to_string())))
                .filter(|s| !s.trim().is_empty())
                .collect();
            Some(PendingAfterTools {
                remaining_tool_ids: res_ids,
                deferred_blocks,
                top_level_reasoning: reasoning_content.map(|s| s.to_string()),
                reasoning_replay,
                deferred_emitted: false,
            })
        } else {
            None
        };

        (vec![pre_msg], pnd)
    }

    pub fn convert_assistant_message(
        content: &[ContentBlock],
        reasoning_content: Option<&str>,
        reasoning_replay: ReasoningReplayMode,
    ) -> Vec<Value> {
        let mut content_parts: Vec<String> = Vec::new();
        let mut thinking_parts: Vec<String> = Vec::new();
        let mut tool_calls: Vec<Value> = Vec::new();

        for block in content {
            match block {
                ContentBlock::Text(t) => {
                    content_parts.push(t.text.clone());
                }
                ContentBlock::Thinking(th) => {
                    if reasoning_replay != ReasoningReplayMode::Disabled {
                        if reasoning_replay == ReasoningReplayMode::ThinkTags {
                            content_parts.push(think_tag_content(&th.thinking));
                        } else if reasoning_content.is_none() {
                            thinking_parts.push(th.thinking.clone());
                        }
                    }
                }
                ContentBlock::RedactedThinking(_) => {}
                ContentBlock::ToolUse(tool_use) => {
                    let args = serde_json::to_string(&tool_use.input).unwrap_or_default();
                    tool_calls.push(serde_json::json!({
                        "id": tool_use.id,
                        "type": "function",
                        "function": {
                            "name": tool_use.name,
                            "arguments": args,
                        },
                    }));
                }
                _ => {
                    let _ = assert_no_forbidden_assistant_block(block);
                }
            }
        }

        let content_str = if content_parts.is_empty() && tool_calls.is_empty() {
            String::from(" ")
        } else {
            content_parts.join("\n\n")
        };

        let mut msg = serde_json::json!({
            "role": "assistant",
            "content": content_str,
        });

        if !tool_calls.is_empty() {
            msg["tool_calls"] = serde_json::json!(tool_calls);
        }

        if reasoning_replay == ReasoningReplayMode::ReasoningContent {
            let replay_reasoning = reasoning_content.map(|s| s.to_string()).or_else(|| {
                if thinking_parts.is_empty() {
                    None
                } else {
                    Some(thinking_parts.join("\n"))
                }
            });
            if let Some(rr) = replay_reasoning {
                msg["reasoning_content"] = Value::String(rr);
            }
        }

        vec![msg]
    }

    pub fn deferred_post_tool_to_messages(pending: &PendingAfterTools) -> Vec<Value> {
        if pending.deferred_blocks.is_empty() {
            return Vec::new();
        }
        Self::convert_assistant_message(
            &pending.deferred_blocks,
            pending.top_level_reasoning.as_deref(),
            pending.reasoning_replay.clone(),
        )
    }

    pub fn convert_user_message_with_injection(
        content: &[ContentBlock],
        pending: &PendingAfterTools,
    ) -> Value {
        if !pending.needs_deferred() || pending.remaining_tool_ids.is_empty() {
            return serde_json::json!({
                "messages": Self::convert_user_message(content),
                "cleared_pending": false,
            });
        }

        let mut result: Vec<Value> = Vec::new();
        let mut text_parts: Vec<String> = Vec::new();
        let mut cleared = false;

        for block in content {
            match block {
                ContentBlock::Text(t) => {
                    text_parts.push(t.text.clone());
                }
                ContentBlock::Image(_) => {
                    return serde_json::json!({
                        "messages": result,
                        "cleared_pending": false,
                        "error": "User message image blocks are not supported for OpenAI chat conversion."
                    });
                }
                ContentBlock::ToolResult(tr) => {
                    if !text_parts.is_empty() {
                        result.push(serde_json::json!({
                            "role": "user",
                            "content": text_parts.join("\n")
                        }));
                        text_parts.clear();
                    }
                    let serialized = serialize_tool_result_content(&tr.content);
                    result.push(serde_json::json!({
                        "role": "tool",
                        "tool_call_id": tr.tool_use_id,
                        "content": serialized,
                    }));

                    if pending.remaining_tool_ids.is_empty() {
                        result.extend(Self::deferred_post_tool_to_messages(pending));
                        cleared = true;
                    }
                }
                _ => {}
            }
        }

        if !text_parts.is_empty() {
            result.push(serde_json::json!({
                "role": "user",
                "content": text_parts.join("\n")
            }));
        }

        serde_json::json!({
            "messages": result,
            "cleared_pending": cleared,
        })
    }

    pub fn convert_user_message(content: &[ContentBlock]) -> Vec<Value> {
        let mut result: Vec<Value> = Vec::new();
        let mut text_parts: Vec<String> = Vec::new();

        for block in content {
            match block {
                ContentBlock::Text(t) => {
                    text_parts.push(t.text.clone());
                }
                ContentBlock::ToolResult(tr) => {
                    if !text_parts.is_empty() {
                        result.push(serde_json::json!({
                            "role": "user",
                            "content": text_parts.join("\n")
                        }));
                        text_parts.clear();
                    }
                    let serialized = serialize_tool_result_content(&tr.content);
                    result.push(serde_json::json!({
                        "role": "tool",
                        "tool_call_id": tr.tool_use_id,
                        "content": serialized,
                    }));
                }
                _ => {}
            }
        }

        if !text_parts.is_empty() {
            result.push(serde_json::json!({
                "role": "user",
                "content": text_parts.join("\n")
            }));
        }

        result
    }

    pub fn convert_tools(tools: &[Tool]) -> Vec<Value> {
        tools
            .iter()
            .map(|tool| {
                let input_schema = tool
                    .input_schema
                    .as_ref()
                    .map(|s| serde_json::to_value(s).unwrap_or_else(|_| serde_json::json!({})))
                    .unwrap_or_else(|| serde_json::json!({"type": "object", "properties": {}}));
                serde_json::json!({
                    "type": "function",
                    "function": {
                        "name": tool.name,
                        "description": tool.description.as_deref().unwrap_or(""),
                        "parameters": input_schema,
                    },
                })
            })
            .collect()
    }

    pub fn convert_tool_choice(tool_choice: &Value) -> Value {
        if !tool_choice.is_object() {
            return tool_choice.clone();
        }

        let choice_type = tool_choice.get("type").and_then(|v| v.as_str());
        match choice_type {
            Some("tool") => {
                if let Some(name) = tool_choice.get("name").and_then(|v| v.as_str()) {
                    return serde_json::json!({
                        "type": "function",
                        "function": { "name": name }
                    });
                }
            }
            Some("any") => return serde_json::json!("required"),
            Some("auto") | Some("none") | Some("required") => {
                return Value::String(choice_type.unwrap().to_string());
            }
            Some("function") => {
                if tool_choice.get("function").map(|v| v.is_object()).unwrap_or(false) {
                    return tool_choice.clone();
                }
            }
            _ => {}
        }

        tool_choice.clone()
    }

    pub fn convert_system_prompt(system: &SystemContentOrString) -> Option<Value> {
        match system {
            SystemContentOrString::String(s) => {
                if !s.is_empty() {
                    Some(serde_json::json!({ "role": "system", "content": s }))
                } else {
                    None
                }
            }
            SystemContentOrString::System(sys) => {
                if !sys.text.is_empty() {
                    Some(serde_json::json!({ "role": "system", "content": sys.text }))
                } else {
                    None
                }
            }
        }
    }
}

pub fn convert_anthropic_to_openai(messages: &[Message]) -> Value {
    let mut openai_messages = Vec::new();

    for message in messages {
        let role = match message.role {
            Role::User => "user",
            Role::Assistant => "assistant",
            Role::System => "system",
        };

        let content = match &message.content {
            ContentOrBlocks::String(s) => {
                serde_json::json!([{ "type": "text", "text": s }])
            }
            ContentOrBlocks::Blocks(blocks) => {
                let arr: Vec<Value> = blocks.iter().map(block_to_json).collect();
                Value::Array(arr)
            }
        };

        let mut msg = serde_json::json!({ "role": role, "content": content });
        if let Some(rc) = &message.reasoning_content {
            msg["reasoning_content"] = Value::String(rc.clone());
        }
        openai_messages.push(msg);
    }

    serde_json::json!({ "messages": openai_messages })
}

pub fn convert_openai_to_anthropic(openai_response: &Value) -> Result<MessagesResponse, String> {
    let id = openai_response
        .get("id")
        .and_then(|v| v.as_str())
        .unwrap_or("msg_000000000000000000000000");

    let model = openai_response
        .get("model")
        .and_then(|v| v.as_str())
        .unwrap_or("gpt-3.5-turbo");

    let empty = Vec::new();
    let choices = openai_response
        .get("choices")
        .and_then(|v| v.as_array())
        .unwrap_or(&empty);

    let mut content = Vec::new();
    for choice in choices {
        if let Some(message) = choice.get("message") {
            if let Some(content_array) = message.get("content").and_then(|v| v.as_array()) {
                for item in content_array {
                    content.push(item.clone());
                }
            }
        }
    }

    let usage = openai_response
        .get("usage")
        .and_then(|v| v.as_object())
        .map(|obj| Usage {
            input_tokens: obj.get("prompt_tokens").and_then(|v| v.as_i64()).unwrap_or(0) as i32,
            output_tokens: obj.get("completion_tokens").and_then(|v| v.as_i64()).unwrap_or(0) as i32,
            cache_creation_input_tokens: obj.get("cache_creation_input_tokens").and_then(|v| v.as_i64()).unwrap_or(0) as i32,
            cache_read_input_tokens: obj.get("cache_read_input_tokens").and_then(|v| v.as_i64()).unwrap_or(0) as i32,
        })
        .unwrap_or_else(|| Usage {
            input_tokens: 0,
            output_tokens: 0,
            cache_creation_input_tokens: 0,
            cache_read_input_tokens: 0,
        });

    Ok(MessagesResponse::new(
        id.to_string(),
        model.to_string(),
        content,
        usage,
    ))
}

pub fn build_base_request_body(
    request_data: &MessagesRequest,
    default_max_tokens: Option<i32>,
    reasoning_replay: ReasoningReplayMode,
) -> Value {
    let mut messages = AnthropicToOpenAIConverter::convert_messages(
        &request_data.messages,
        reasoning_replay,
    );

    if let Some(system) = &request_data.system {
        if let Some(system_msg) = AnthropicToOpenAIConverter::convert_system_prompt(system) {
            messages.insert(0, system_msg);
        }
    }

    let mut body = serde_json::json!({
        "model": request_data.model,
        "messages": messages,
    });

    if let Some(obj) = body.as_object_mut() {
        set_if_not_none(obj, "max_tokens", request_data.max_tokens.or(default_max_tokens));
        set_if_not_none(obj, "temperature", request_data.temperature);
        set_if_not_none(obj, "top_p", request_data.top_p);

        if let Some(stop_sequences) = &request_data.stop_sequences {
            obj.insert("stop".to_string(), serde_json::json!(stop_sequences));
        }

        if let Some(tools) = &request_data.tools {
            obj.insert("tools".to_string(), serde_json::json!(tools));
        }

        if let Some(tool_choice) = &request_data.tool_choice {
            obj.insert(
                "tool_choice".to_string(),
                AnthropicToOpenAIConverter::convert_tool_choice(tool_choice),
            );
        }
    }

    body
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_reasoning_replay_mode_default() {
        assert_eq!(ReasoningReplayMode::default(), ReasoningReplayMode::ThinkTags);
    }

    #[test]
    fn test_pending_after_tools_needs_deferred() {
        let mut pending = PendingAfterTools::new();
        assert!(!pending.needs_deferred());

        pending.deferred_blocks.push(ContentBlock::Text(crate::api::models::TextContent {
            text: "test".to_string(),
            extra: HashMap::new(),
        }));
        assert!(pending.needs_deferred());

        pending.deferred_emitted = true;
        assert!(!pending.needs_deferred());
    }

    #[test]
    fn test_think_tag_content() {
        assert_eq!(think_tag_content("hello"), "<thinking>\nhello\n</thinking>");
    }

    #[test]
    fn test_serialize_tool_result_content_string() {
        assert_eq!(
            serialize_tool_result_content(&Value::String("test".to_string())),
            "test"
        );
    }

    #[test]
    fn test_serialize_tool_result_content_dict() {
        let content = serde_json::json!({"key": "value"});
        let result = serialize_tool_result_content(&content);
        assert!(result.contains("key"));
        assert!(result.contains("value"));
    }

    #[test]
    fn test_serialize_tool_result_content_list() {
        let content = serde_json::json!([
            {"type": "text", "text": "hello"},
            {"type": "text", "text": "world"}
        ]);
        assert_eq!(serialize_tool_result_content(&content), "hello\nworld");
    }

    #[test]
    fn test_convert_tools() {
        let tools = vec![Tool {
            name: "test_tool".to_string(),
            type_field: None,
            description: Some("A test tool".to_string()),
            input_schema: Some(HashMap::from([("type".to_string(), serde_json::json!("object"))])),
            extra: HashMap::new(),
        }];
        let result = AnthropicToOpenAIConverter::convert_tools(&tools);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0]["type"], "function");
    }

    #[test]
    fn test_convert_tool_choice_tool_type() {
        let tool_choice = serde_json::json!({
            "type": "tool",
            "name": "test_func"
        });
        let result = AnthropicToOpenAIConverter::convert_tool_choice(&tool_choice);
        assert_eq!(result["type"], "function");
        assert_eq!(result["function"]["name"], "test_func");
    }

    #[test]
    fn test_convert_system_prompt_string() {
        let system = SystemContentOrString::String("You are helpful".to_string());
        let result = AnthropicToOpenAIConverter::convert_system_prompt(&system);
        assert!(result.is_some());
        assert_eq!(result.unwrap()["role"], "system");
    }

    #[test]
    fn test_block_type_text() {
        let block = ContentBlock::Text(crate::api::models::TextContent {
            text: "hello".to_string(),
            extra: HashMap::new(),
        });
        assert_eq!(block_type(&block), "text");
    }

    #[test]
    fn test_block_type_tool_use() {
        let block = ContentBlock::ToolUse(crate::api::models::ToolUseContent {
            id: "123".to_string(),
            name: "test".to_string(),
            input: HashMap::new(),
            extra: HashMap::new(),
        });
        assert_eq!(block_type(&block), "tool_use");
    }
}