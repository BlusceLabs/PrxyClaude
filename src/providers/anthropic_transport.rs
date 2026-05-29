//! Anthropic Messages transport base class
//!
//! Port of `providers/anthropic_messages.py` to Rust.

use reqwest::StatusCode;
use serde_json::Value;
use std::sync::Arc;
use tracing::{info, warn};

use crate::core::anthropic::emitted_sse_tracker::EmittedNativeSseTracker;
use crate::core::anthropic::native_messages_request::NativeMessagesRequest;
use crate::core::anthropic::native_sse_block_policy::{
    NativeSseBlockPolicyState, transform_native_sse_block_event,
};
use crate::models::MessagesRequest;
use crate::providers::error_mapping::user_visible_message_for_mapped_provider_error;
use crate::providers::exceptions::ProviderError;
use crate::providers::model_listing::extract_openai_model_ids;
use crate::providers::rate_limit::GlobalRateLimiter;

/// Default max output tokens for Anthropic messages endpoint.
const ANTHROPIC_DEFAULT_MAX_OUTPUT_TOKENS: i32 = 8192;

/// Max bytes to log from error response body.
const NATIVE_MESSAGES_ERROR_BODY_LOG_CAP_BYTES: usize = 4096;

/// Configuration for an Anthropic messages transport.
#[derive(Debug, Clone)]
pub struct AnthropicTransportConfig {
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
    pub log_api_error_tracebacks: bool,
}

impl Default for AnthropicTransportConfig {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            base_url: "https://api.anthropic.com".to_string(),
            rate_limit: None,
            rate_window: None,
            max_concurrency: 10,
            http_read_timeout: Some(300.0),
            http_connect_timeout: Some(10.0),
            http_write_timeout: Some(10.0),
            proxy: None,
            log_raw_sse_events: false,
            log_api_error_tracebacks: false,
        }
    }
}

/// Transport for providers that stream from an Anthropic-compatible endpoint.
pub struct AnthropicMessagesTransport {
    provider_name: String,
    api_key: String,
    base_url: String,
    config: AnthropicTransportConfig,
    rate_limiter: Arc<GlobalRateLimiter>,
    client: reqwest::Client,
}

impl AnthropicMessagesTransport {
    pub fn new(config: AnthropicTransportConfig, provider_name: &str) -> Self {
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
        let response = self.send_model_list_request().await?;
        let payload = self.model_list_json(response).await?;
        self.extract_model_ids(payload)
    }

    async fn send_model_list_request(&self) -> Result<reqwest::Response, ProviderError> {
        let url = format!("{}/models", self.base_url);
        self.client
            .get(&url)
            .headers(self.model_list_headers())
            .send()
            .await
            .map_err(|e| ProviderError::api_error(&e.to_string(), 503))
    }

    fn model_list_headers(&self) -> reqwest::header::HeaderMap {
        let mut headers = reqwest::header::HeaderMap::new();
        if !self.api_key.is_empty() {
            headers.insert("x-api-key", self.api_key.parse().unwrap());
        }
        headers.insert("anthropic-version", "2023-06-01".parse().unwrap());
        headers
    }

    async fn model_list_json(&self, response: reqwest::Response) -> Result<Value, ProviderError> {
        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(ProviderError::api_error(
                &format!("HTTP {} {}", status, body),
                status.as_u16(),
            ));
        }
        response
            .json::<Value>()
            .await
            .map_err(|e| ProviderError::api_error(&format!("Invalid JSON: {}", e), 503))
    }

    fn extract_model_ids(&self, payload: Value) -> Result<Vec<String>, ProviderError> {
        extract_openai_model_ids(&payload, &self.provider_name)
    }

    fn request_headers(&self) -> reqwest::header::HeaderMap {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert("content-type", "application/json".parse().unwrap());
        headers.insert("x-api-key", self.api_key.parse().unwrap());
        headers.insert("anthropic-version", "2023-06-01".parse().unwrap());
        headers
    }

    fn build_request_body(
        &self,
        request: &MessagesRequest,
        thinking_enabled: Option<bool>,
    ) -> Value {
        let thinking = thinking_enabled.unwrap_or(false);
        let native_request = NativeMessagesRequest::from_request(request);

        let mut body = serde_json::json!({
            "model": native_request.model,
            "messages": native_request.messages,
            "max_tokens": native_request.max_tokens.unwrap_or(ANTHROPIC_DEFAULT_MAX_OUTPUT_TOKENS),
        });

        if let Some(system) = &native_request.system {
            if !system.is_empty() {
                body["system"] = Value::String(system.clone());
            }
        }

        if let Some(temp) = native_request.temperature {
            body["temperature"] = Value::Number(serde_json::Number::from_f64(temp).unwrap());
        }

        if let Some(top_p) = native_request.top_p {
            body["top_p"] = Value::Number(serde_json::Number::from_f64(top_p).unwrap());
        }

        if let Some(stop) = &native_request.stop_sequences {
            body["stop_sequences"] = serde_json::json!(stop);
        }

        if thinking {
            body["thinking"] = serde_json::json!({
                "type": "enabled",
                "budget_tokens": ANTHROPIC_DEFAULT_MAX_OUTPUT_TOKENS as u64 / 2,
            });
        }

        body
    }

    async fn send_stream_request(&self, body: &Value) -> Result<reqwest::Response, ProviderError> {
        let url = format!("{}/messages", self.base_url);
        self.client
            .post(&url)
            .headers(self.request_headers())
            .json(body)
            .send()
            .await
            .map_err(|e| ProviderError::api_error(&e.to_string(), 503))
    }

    fn get_error_message(&self, error: &ProviderError, request_id: Option<&str>) -> String {
        let base_message =
            user_visible_message_for_mapped_provider_error(error, &self.provider_name, None);
        self.format_error_message(&base_message, request_id)
    }

    fn format_error_message(&self, base_message: &str, request_id: Option<&str>) -> String {
        if let Some(id) = request_id {
            format!("{}\nRequest ID: {}", base_message, id)
        } else {
            base_message.to_string()
        }
    }

    fn log_stream_transport_error(&self, error: &ProviderError) {
        warn!(
            "{}_STREAM: transport error: {}",
            self.provider_name, error
        );
    }

    /// Stream response via a native Anthropic-compatible messages endpoint.
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

        let body = self.build_request_body(request, thinking_enabled);
        let thinking = thinking_enabled.unwrap_or(false);

        info!(
            "{}_STREAM:{} natively passing Anthropic request model={} msgs={} tools={}",
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

        let mut sent_any_event = false;
        let mut emitted_tracker = EmittedNativeSseTracker::new();
        let mut state = NativeSseBlockPolicyState::new();
        let mut all_events = Vec::new();

        let _permit = self.rate_limiter.concurrency_slot().await;

        let result = self
            .send_with_retry(&body, &mut sent_any_event, &mut emitted_tracker, &mut state, thinking)
            .await;

        match result {
            Ok(events) => all_events.extend(events),
            Err(error) => {
                self.log_stream_transport_error(&error);
                let error_message = self.get_error_message(&error, request_id);

                info!(
                    "{}_STREAM: Emitting native SSE error event for {}{}",
                    tag,
                    std::any::type_name_of_val(&error),
                    req_tag,
                );

                if sent_any_event {
                    for event in emitted_tracker.close_unclosed_blocks() {
                        all_events.push(format_sse_value(event));
                    }
                    for event in emitted_tracker.midstream_error_tail(&error_message, input_tokens) {
                        all_events.push(format_sse_value(event));
                    }
                } else {
                    // Emit error event directly
                    let error_event = serde_json::json!({
                        "type": "error",
                        "error": {
                            "type": "api_error",
                            "message": error_message,
                        },
                    });
                    all_events.push(format_sse_value(error_event));
                    all_events.push("event: message_stop\ndata: {\"type\":\"message_stop\"}\n\n".to_string());
                }
            }
        }

        Ok(all_events)
    }

    async fn send_with_retry(
        &self,
        body: &Value,
        sent_any_event: &mut bool,
        emitted_tracker: &mut EmittedNativeSseTracker,
        state: &mut NativeSseBlockPolicyState,
        thinking: bool,
    ) -> Result<Vec<String>, ProviderError> {
        let response = self.send_stream_request(body).await?;
        let status = response.status();

        if status == StatusCode::TOO_MANY_REQUESTS {
            return Err(ProviderError::rate_limit("Rate limited"));
        }

        if !status.is_success() {
            return Err(ProviderError::api_error(
                &format!("HTTP {}", status),
                status.as_u16(),
            ));
        }

        let mut events = Vec::new();
        let response = response;
        let mut bytes_stream = response.bytes_stream();
        use futures::StreamExt;
        let mut line_buf = String::new();
        let mut event_lines: Vec<String> = Vec::new();

        while let Some(chunk_result) = bytes_stream.next().await {
            let chunk = match chunk_result {
                Ok(c) => c,
                Err(e) => {
                    return Err(ProviderError::api_error(&e.to_string(), 503));
                }
            };

            line_buf.push_str(&String::from_utf8_lossy(&chunk));

            while let Some(newline_pos) = line_buf.find('\n') {
                let line = line_buf[..newline_pos].to_string();
                line_buf = line_buf[newline_pos + 1..].to_string();

                if line.trim().is_empty() {
                    if !event_lines.is_empty() {
                        let event_text = event_lines.join("\n") + "\n\n";
                        event_lines.clear();

                        *sent_any_event = true;
                        emitted_tracker.feed(&event_text);

                        let output_event =
                            transform_native_sse_block_event(&event_text, state, thinking);
                        if let Some(transformed) = output_event {
                            events.push(transformed);
                        }
                    }
                } else {
                    event_lines.push(line);
                }
            }
        }

        // Process remaining event lines
        if !event_lines.is_empty() {
            let event_text = event_lines.join("\n") + "\n\n";
            *sent_any_event = true;
            emitted_tracker.feed(&event_text);

            let output_event =
                transform_native_sse_block_event(&event_text, state, thinking);
            if let Some(transformed) = output_event {
                events.push(transformed);
            }
        }

        Ok(events)
    }
}

fn format_sse_value(value: Value) -> String {
    if let Some(obj) = value.as_object() {
        let event_type = obj
            .get("type")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");
        let data = serde_json::to_string(obj).unwrap_or_default();
        format!("event: {}\ndata: {}\n\n", event_type, data)
    } else {
        format!("data: {}\n\n", value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_sse_value() {
        let value = serde_json::json!({
            "type": "message_start",
            "message": {"id": "msg_123"}
        });
        let formatted = format_sse_value(value);
        assert!(formatted.contains("event: message_start"));
        assert!(formatted.contains("message_start"));
    }

    #[test]
    fn test_anthropic_transport_config_default() {
        let config = AnthropicTransportConfig::default();
        assert_eq!(config.base_url, "https://api.anthropic.com");
        assert_eq!(config.max_concurrency, 10);
    }

    #[test]
    fn test_format_error_message_with_request_id() {
        let config = AnthropicTransportConfig::default();
        let transport = AnthropicMessagesTransport::new(config, "test_provider");

        let msg = transport.format_error_message("error occurred", Some("req_123"));
        assert!(msg.contains("req_123"));
        assert!(msg.contains("error occurred"));
    }

    #[test]
    fn test_format_error_message_without_request_id() {
        let config = AnthropicTransportConfig::default();
        let transport = AnthropicMessagesTransport::new(config, "test_provider");

        let msg = transport.format_error_message("error occurred", None);
        assert_eq!(msg, "error occurred");
    }
}
