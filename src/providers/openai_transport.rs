//! OpenAI Chat transport base class
//!
//! Port of `providers/openai_compat.py` to Rust.

use serde_json::Value;
use std::sync::Arc;
use tracing::{debug, error, info};

use crate::core::anthropic::conversion::{ReasoningReplayMode, build_base_request_body};
use crate::core::anthropic::sse::SSEBuilder;
use crate::core::anthropic::thinking::{ContentType, ThinkTagParser};
use crate::core::anthropic::tools::HeuristicToolParser;
use crate::core::anthropic::utils::append_request_id;
use crate::models::MessagesRequest;
use crate::providers::error_mapping::user_visible_message_for_mapped_provider_error;
use crate::providers::exceptions::ProviderError;
use crate::providers::model_listing::extract_openai_model_ids;
use crate::providers::rate_limit::GlobalRateLimiter;

/// Configuration for an OpenAI chat transport.
#[derive(Debug, Clone)]
pub struct OpenAITransportConfig {
    pub api_key: String,
    pub base_url: String,
    pub rate_limit: Option<usize>,
    pub rate_window: Option<f64>,
    pub max_concurrency: usize,
    pub http_read_timeout: Option<f64>,
    pub http_connect_timeout: Option<f64>,
    pub http_write_timeout: Option<f64>,
    pub proxy: Option<String>,
    pub log_raw_sse_events: bool,
}

impl Default for OpenAITransportConfig {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            base_url: "https://api.openai.com/v1".to_string(),
            rate_limit: None,
            rate_window: None,
            max_concurrency: 10,
            http_read_timeout: Some(300.0),
            http_connect_timeout: Some(10.0),
            http_write_timeout: Some(10.0),
            proxy: None,
            log_raw_sse_events: false,
        }
    }
}

/// Transport for OpenAI-compatible `/chat/completions` adapters.
pub struct OpenAIChatTransport {
    provider_name: String,
    api_key: String,
    base_url: String,
    config: OpenAITransportConfig,
    rate_limiter: Arc<GlobalRateLimiter>,
    client: reqwest::Client,
}

impl OpenAIChatTransport {
    pub fn new(config: OpenAITransportConfig, provider_name: &str) -> Self {
        let base_url = config.base_url.trim_end_matches('/').to_string();
        let rate_limiter = GlobalRateLimiter::get_scoped_instance(
            &provider_name.to_lowercase(),
            config.rate_limit,
            config.rate_window,
            config.max_concurrency,
        );

        let mut client_builder = reqwest::Client::builder();
        if let Some(ref proxy) = config.proxy {
            client_builder = client_builder.proxy(reqwest::Proxy::all(proxy).unwrap());
        }
        let client = client_builder.build().expect("Failed to build HTTP client");

        Self {
            provider_name: provider_name.to_string(),
            api_key: config.api_key.clone(),
            base_url,
            config,
            rate_limiter,
            client,
        }
    }

    pub async fn cleanup(&self) {
        // reqwest::Client doesn't require explicit cleanup in Rust
    }

    pub async fn list_model_ids(&self) -> Result<Vec<String>, ProviderError> {
        let url = format!("{}/models", self.base_url);
        let response = self
            .client
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .send()
            .await
            .map_err(|e| ProviderError::api_error(&e.to_string(), 503))?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(ProviderError::api_error(
                &format!("HTTP {} {}", status, body),
                status.as_u16(),
            ));
        }

        let payload: Value = response
            .json()
            .await
            .map_err(|e| ProviderError::api_error(&format!("Invalid JSON: {}", e), 503))?;

        extract_openai_model_ids(&payload, &self.provider_name)
    }

    fn build_request_headers(&self) -> reqwest::header::HeaderMap {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert("content-type", "application/json".parse().unwrap());
        if !self.api_key.is_empty() {
            headers.insert(
                "Authorization",
                format!("Bearer {}", self.api_key).parse().unwrap(),
            );
        }
        headers
    }

    fn build_base_request_body(&self, request: &MessagesRequest, thinking_enabled: Option<bool>) -> Value {
        let reasoning_replay = if thinking_enabled.unwrap_or(false) {
            ReasoningReplayMode::ThinkTags
        } else {
            ReasoningReplayMode::Disabled
        };
        build_base_request_body(request, None, reasoning_replay)
    }

    fn handle_extra_reasoning(
        &self,
        _delta: &Value,
        _sse: &mut SSEBuilder,
        _thinking_enabled: bool,
    ) -> Vec<String> {
        // Hook for provider-specific reasoning (e.g. OpenRouter reasoning_details)
        Vec::new()
    }

    async fn create_stream(
        &self,
        body: &mut Value,
    ) -> Result<(reqwest::Response, Value), ProviderError> {
        if let Some(obj) = body.as_object_mut() {
            obj.insert("stream".to_string(), Value::Bool(true));
            obj.insert(
                "stream_options".to_string(),
                serde_json::json!({"include_usage": true}),
            );
        }

        let url = format!("{}/chat/completions", self.base_url);
        let response = self
            .client
            .post(&url)
            .headers(self.build_request_headers())
            .json(body)
            .send()
            .await
            .map_err(|e| ProviderError::api_error(&e.to_string(), 503))?;

        let status = response.status();
        if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            return Err(ProviderError::rate_limit("Rate limited"));
        }

        if !status.is_success() {
            let error_body = response.text().await.unwrap_or_default();
            return Err(ProviderError::api_error(
                &format!("HTTP {}: {}", status, error_body),
                status.as_u16(),
            ));
        }

        Ok((response, body.clone()))
    }

    fn emit_tool_arg_delta(
        &self,
        sse: &mut SSEBuilder,
        tc_index: i32,
        args: &str,
    ) -> Vec<String> {
        if args.is_empty() {
            return Vec::new();
        }
        let state = match sse.blocks.tool_states.get(&tc_index) {
            Some(s) => s,
            None => return Vec::new(),
        };

        if state.name == "Task" {
            let parsed = Self::buffer_task_args(sse, tc_index, args);
            if let Some(p) = parsed {
                return vec![sse.emit_tool_delta(tc_index, &p)];
            }
            return Vec::new();
        }

        vec![sse.emit_tool_delta(tc_index, args)]
    }

    fn buffer_task_args(sse: &mut SSEBuilder, tc_index: i32, args: &str) -> Option<String> {
        let state = sse.blocks.tool_states.get_mut(&tc_index)?;
        state.task_arg_buffer.push_str(args);

        // Try to parse the accumulated buffer
        match serde_json::from_str::<Value>(&state.task_arg_buffer) {
            Ok(parsed) => Some(serde_json::to_string(&parsed).unwrap_or_default()),
            Err(_) => None,
        }
    }

    fn process_tool_call(&self, tc: &Value, sse: &mut SSEBuilder) -> Vec<String> {
        let mut events = Vec::new();
        let tc_index = tc.get("index").and_then(|v| v.as_i64()).unwrap_or(0);
        let tc_index = if tc_index < 0 {
            sse.blocks.tool_states.len() as i32
        } else {
            tc_index as i32
        };

        let fn_delta = tc.get("function").cloned().unwrap_or(Value::Object(Default::default()));
        let incoming_name = fn_delta.get("name").and_then(|v| v.as_str());
        let arguments = fn_delta
            .get("arguments")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        if let Some(id) = tc.get("id").and_then(|v| v.as_str()) {
            if !id.is_empty() {
                sse.blocks.set_stream_tool_id(tc_index, id);
            }
        }

        if let Some(name) = incoming_name {
            sse.blocks.register_tool_name(tc_index, name);
        }

        // Extract state info before mutable operations
        let (resolved_id, resolved_name, needs_start) = {
            let state = sse.blocks.tool_states.get(&tc_index);
            let resolved_id = state
                .and_then(|s| {
                    if s.tool_id.is_empty() {
                        None
                    } else {
                        Some(s.tool_id.clone())
                    }
                })
                .or_else(|| tc.get("id").and_then(|v| v.as_str()).map(|s| s.to_string()));
            let resolved_name = state
                .map(|s| s.name.as_str())
                .unwrap_or("")
                .to_string();
            let needs_start = state.is_none() || !state.unwrap().started;
            (resolved_id, resolved_name, needs_start)
        };

        if needs_start {
            let name_ok = !resolved_name.trim().is_empty();
            if name_ok {
                let tool_id = resolved_id.unwrap_or_else(|| {
                    format!("tool_{}", uuid::Uuid::new_v4())
                });
                let display_name = if resolved_name.trim().is_empty() {
                    "tool_call"
                } else {
                    resolved_name.trim()
                };
                events.push(sse.start_tool_block(tc_index, &tool_id, display_name));

                // Emit pre-start args if any
                if let Some(state) = sse.blocks.tool_states.get_mut(&tc_index) {
                    if !state.pre_start_args.is_empty() {
                        let pre = state.pre_start_args.clone();
                        state.pre_start_args.clear();
                        events.extend(self.emit_tool_arg_delta(sse, tc_index, &pre));
                    }
                }
            }
        }

        // Re-check state after potential start
        let still_needs_init = {
            let state = sse.blocks.tool_states.get(&tc_index);
            state.is_none() || !state.unwrap().started
        };

        if arguments.is_empty() {
            return events;
        }
        if still_needs_init {
            sse.blocks.ensure_tool_state(tc_index);
            if resolved_name.trim().is_empty() {
                if let Some(state) = sse.blocks.tool_states.get_mut(&tc_index) {
                    state.pre_start_args.push_str(arguments);
                }
                return events;
            }
        }

        events.extend(self.emit_tool_arg_delta(sse, tc_index, arguments));
        events
    }

    fn flush_task_arg_buffers(&self, sse: &mut SSEBuilder) -> Vec<String> {
        let mut events = Vec::new();
        let flushed = sse.blocks.flush_task_arg_buffers();
        for (tool_index, args) in flushed {
            events.push(sse.emit_tool_delta(tool_index, &args));
        }
        events
    }

    /// Stream response in Anthropic SSE format.
    pub async fn stream_response(
        &self,
        request: &MessagesRequest,
        input_tokens: i32,
        request_id: Option<&str>,
        thinking_enabled: Option<bool>,
    ) -> Result<Vec<String>, ProviderError> {
        let tag = &self.provider_name;
        let req_tag = request_id
            .map(|id| format!(" request_id={}", id))
            .unwrap_or_default();
        let message_id = format!("msg_{}", uuid::Uuid::new_v4());
        let model = request.model.clone();

        let mut sse = SSEBuilder::new(
            message_id,
            model,
            input_tokens,
            self.config.log_raw_sse_events,
        );

        let mut body = self.build_base_request_body(request, thinking_enabled);
        let thinking = thinking_enabled.unwrap_or(false);

        info!(
            "{}_STREAM:{} model={} msgs={} tools={}",
            tag,
            req_tag,
            body.get("model").and_then(|v| v.as_str()).unwrap_or(""),
            body.get("messages")
                .and_then(|v| v.as_array())
                .map(|a| a.len())
                .unwrap_or(0),
            body.get("tools")
                .and_then(|v| v.as_array())
                .map(|a| a.len())
                .unwrap_or(0),
        );

        let mut all_events = Vec::new();
        all_events.push(sse.message_start());

        let mut think_parser = ThinkTagParser::new();
        let mut heuristic_parser = HeuristicToolParser::new();
        let mut finish_reason: Option<String> = None;
        let mut usage_info: Option<Value> = None;

        let _permit = self.rate_limiter.concurrency_slot().await;

        let stream_result = self.create_stream(&mut body).await;

        match stream_result {
            Ok((response, _body)) => {
                use futures::StreamExt;
                let mut bytes_stream = response.bytes_stream();
                let mut buf = String::new();

                while let Some(chunk_result) = bytes_stream.next().await {
                    let chunk = match chunk_result {
                        Ok(c) => c,
                        Err(e) => {
                            error!("{}_STREAM: network error: {}", tag, e);
                            let error_message = format!("Network error: {}", e);
                            let error_message = self.format_error_message(&error_message, request_id);
                            all_events.extend(sse.close_all_blocks());
                            all_events.push(sse.emit_top_level_error(&error_message));
                            all_events.push(sse.message_delta("end_turn", Some(1)));
                            all_events.push(sse.message_stop());
                            return Ok(all_events);
                        }
                    };

                    buf.push_str(&String::from_utf8_lossy(&chunk));

                    // Parse SSE frames
                    while let Some(double_newline) = buf.find("\n\n") {
                        let frame = buf[..double_newline].to_string();
                        buf = buf[double_newline + 2..].to_string();

                        for line in frame.lines() {
                            if let Some(data) = line.strip_prefix("data: ") {
                                if data == "[DONE]" {
                                    continue;
                                }
                                if let Ok(value) = serde_json::from_str::<Value>(data) {
                                    // Check for usage info
                                    if let Some(usage) = value.get("usage") {
                                        usage_info = Some(usage.clone());
                                    }

                                    let choices = value.get("choices").and_then(|v| v.as_array());
                                    let choice = choices
                                        .and_then(|c| c.first())
                                        .cloned()
                                        .unwrap_or(Value::Null);

                                    let delta =
                                        choice.get("delta").cloned().unwrap_or(Value::Null);
                                    if delta.is_null() {
                                        continue;
                                    }

                                    if let Some(fr) =
                                        choice.get("finish_reason").and_then(|v| v.as_str())
                                    {
                                        finish_reason = Some(fr.to_string());
                                        debug!("{} finish_reason: {}", tag, fr);
                                    }

                                    // Handle reasoning_content
                                    let reasoning = delta
                                        .get("reasoning_content")
                                        .and_then(|v| v.as_str());
                                    if thinking && reasoning.is_some() {
                                        let thinking_events = sse.ensure_thinking_block();
                                        all_events.extend(thinking_events);
                                        all_events
                                            .push(sse.emit_thinking_delta(reasoning.unwrap()));
                                    }

                                    // Provider-specific extra reasoning
                                    let extra_events =
                                        self.handle_extra_reasoning(&delta, &mut sse, thinking);
                                    all_events.extend(extra_events);

                                    // Handle text content
                                    if let Some(content) =
                                        delta.get("content").and_then(|v| v.as_str())
                                    {
                                        let parts = think_parser.feed(content);
                                        for part in parts {
                                            if part.content_type == ContentType::Thinking {
                                                if !thinking {
                                                    continue;
                                                }
                                                let thinking_events = sse.ensure_thinking_block();
                                                all_events.extend(thinking_events);
                                                all_events
                                                    .push(sse.emit_thinking_delta(&part.content));
                                            } else {
                                                let (filtered_text, detected_tools) =
                                                    heuristic_parser.feed(&part.content);

                                                if !filtered_text.is_empty() {
                                                    let text_events = sse.ensure_text_block();
                                                    all_events.extend(text_events);
                                                    all_events
                                                        .push(sse.emit_text_delta(&filtered_text));
                                                }

                                                for tool_use in detected_tools {
                                                    let tool_events = emit_heuristic_tool_use_sse(
                                                        &mut sse,
                                                        &tool_use,
                                                    );
                                                    all_events.extend(tool_events);
                                                }
                                            }
                                        }
                                    }

                                    // Handle native tool calls
                                    if let Some(tool_calls) =
                                        delta.get("tool_calls").and_then(|v| v.as_array())
                                    {
                                        let close_events = sse.close_content_blocks();
                                        all_events.extend(close_events);

                                        for tc in tool_calls {
                                            let tc_info = serde_json::json!({
                                                "index": tc.get("index"),
                                                "id": tc.get("id"),
                                                "function": {
                                                    "name": tc.get("function")
                                                        .and_then(|f| f.get("name"))
                                                        .and_then(|v| v.as_str()),
                                                    "arguments": tc.get("function")
                                                        .and_then(|f| f.get("arguments"))
                                                        .and_then(|v| v.as_str()),
                                                },
                                            });
                                            let tc_events = self.process_tool_call(&tc_info, &mut sse);
                                            all_events.extend(tc_events);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                // Flush remaining content
                let remaining = think_parser.flush();
                if let Some(chunk) = remaining {
                    if chunk.content_type == ContentType::Thinking {
                        if thinking {
                            let thinking_events = sse.ensure_thinking_block();
                            all_events.extend(thinking_events);
                            all_events.push(sse.emit_thinking_delta(&chunk.content));
                        }
                    } else {
                        let text_events = sse.ensure_text_block();
                        all_events.extend(text_events);
                        all_events.push(sse.emit_text_delta(&chunk.content));
                    }
                }

                // Flush heuristic tool parser
                let remaining_tools = heuristic_parser.flush();
                for tool_use in remaining_tools {
                    let tool_events = emit_heuristic_tool_use_sse(&mut sse, &tool_use);
                    all_events.extend(tool_events);
                }

                // Check if we have any content blocks
                let has_started_tool = sse
                    .blocks
                    .tool_states
                    .values()
                    .any(|s| s.started);
                let has_content_blocks = sse.blocks.text_index != -1
                    || sse.blocks.thinking_index != -1
                    || has_started_tool;

                if !has_content_blocks {
                    let text_events = sse.ensure_text_block();
                    all_events.extend(text_events);
                    all_events.push(sse.emit_text_delta(" "));
                }

                // Flush task arg buffers
                let task_events = self.flush_task_arg_buffers(&mut sse);
                all_events.extend(task_events);

                // Close all blocks
                let close_events = sse.close_all_blocks();
                all_events.extend(close_events);

                // Calculate output tokens
                let output_tokens = if let Some(ref usage) = usage_info {
                    usage
                        .get("completion_tokens")
                        .and_then(|v| v.as_i64())
                        .map(|t| t as i32)
                        .unwrap_or_else(|| sse.estimate_output_tokens())
                } else {
                    sse.estimate_output_tokens()
                };

                let stop_reason = crate::core::anthropic::sse::map_stop_reason(finish_reason.as_deref());
                all_events.push(sse.message_delta(stop_reason, Some(output_tokens)));
                all_events.push(sse.message_stop());
            }
            Err(e) => {
                let base_message = user_visible_message_for_mapped_provider_error(&e, &self.provider_name, None);
                let error_message = self.format_error_message(&base_message, request_id);

                info!(
                    "{}_STREAM: Emitting SSE error event for {}{}",
                    tag,
                    std::any::type_name_of_val(&e),
                    req_tag,
                );

                let close_events = sse.close_all_blocks();
                all_events.extend(close_events);

                if sse.blocks.has_emitted_tool_block() {
                    all_events.push(sse.emit_top_level_error(&error_message));
                } else {
                    let error_events = sse.emit_error(&error_message);
                    all_events.extend(error_events);
                }

                all_events.push(sse.message_delta("end_turn", Some(1)));
                all_events.push(sse.message_stop());
            }
        }

        Ok(all_events)
    }

    fn format_error_message(&self, base_message: &str, request_id: Option<&str>) -> String {
        match request_id {
            Some(id) if !id.is_empty() => append_request_id(base_message, id),
            _ => base_message.to_string(),
        }
    }
}

/// Emit SSE for one heuristic tool_use block.
fn emit_heuristic_tool_use_sse(sse: &mut SSEBuilder, tool_use: &crate::core::anthropic::tools::ToolUseDetection) -> Vec<String> {
    let mut events = Vec::new();

    // Close open content blocks first
    events.extend(sse.close_content_blocks());

    let block_idx = sse.blocks.allocate_index();
    let _tool_id = &tool_use.id;
    let _name = &tool_use.name;

    events.push(sse.content_block_start(block_idx, "tool_use", None));

    let input_str = serde_json::to_string(&tool_use.input).unwrap_or_default();

    events.push(sse.content_block_delta(block_idx, "input_json_delta", &input_str));
    events.push(sse.content_block_stop(block_idx));

    events
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_openai_transport_config_default() {
        let config = OpenAITransportConfig::default();
        assert_eq!(config.base_url, "https://api.openai.com/v1");
        assert_eq!(config.max_concurrency, 10);
    }

    #[test]
    fn test_build_base_request_body() {
        use crate::api::models::{ContentOrBlocks, Message, Role};

        let config = OpenAITransportConfig::default();
        let transport = OpenAIChatTransport::new(config, "test_provider");

        let request = MessagesRequest {
            model: "gpt-4".to_string(),
            messages: vec![Message {
                role: Role::User,
                content: ContentOrBlocks::String("Hello".to_string()),
                reasoning_content: None,
                extra: std::collections::HashMap::new(),
            }],
            max_tokens: Some(1024),
            system: None,
            temperature: Some(0.7),
            top_p: None,
            stop_sequences: None,
            tools: None,
            tool_choice: None,
            thinking: None,
            stream: None,
            original_model: None,
            resolved_provider_model: None,
            top_k: None,
            metadata: None,
            context_management: None,
            output_config: None,
            mcp_servers: None,
            extra_body: None,
            betas: None,
            extra: std::collections::HashMap::new(),
        };

        let body = transport.build_base_request_body(&request, Some(true));
        assert_eq!(
            body.get("model").and_then(|v| v.as_str()),
            Some("gpt-4")
        );
    }

    #[test]
    fn test_emit_heuristic_tool_use_sse() {
        let mut sse = SSEBuilder::new(
            "msg_test".to_string(),
            "gpt-4".to_string(),
            10,
            false,
        );

        let tool_use = crate::core::anthropic::tools::ToolUseDetection {
            id: "tool_123".to_string(),
            name: "WebFetch".to_string(),
            input: serde_json::json!({"url": "https://example.com"}),
        };

        let events = emit_heuristic_tool_use_sse(&mut sse, &tool_use);
        assert!(!events.is_empty());
    }

    #[test]
    fn test_format_error_message_with_request_id() {
        let config = OpenAITransportConfig::default();
        let transport = OpenAIChatTransport::new(config, "test_provider");

        let msg = transport.format_error_message("error occurred", Some("req_123"));
        assert!(msg.contains("req_123"));
        assert!(msg.contains("error occurred"));
    }

    #[test]
    fn test_format_error_message_without_request_id() {
        let config = OpenAITransportConfig::default();
        let transport = OpenAIChatTransport::new(config, "test_provider");

        let msg = transport.format_error_message("error occurred", None);
        assert_eq!(msg, "error occurred");
    }
}
