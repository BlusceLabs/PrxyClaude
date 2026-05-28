use std::sync::Arc;

use axum::response::{IntoResponse, Response, Sse};
use axum::Json;
use serde_json::Value;
use tokio_stream::StreamExt;

use crate::api::model_router::ModelRouter;
use crate::api::optimization_handlers::try_optimizations;
use crate::api::web_tools::egress::WebFetchEgressPolicy;
use crate::api::web_tools::request::{
    is_web_server_tool_request, openai_chat_upstream_server_tool_error,
};
use crate::api::web_tools::streaming::stream_web_server_tool_response;
use crate::config::Config;
use crate::core::anthropic::errors::get_user_facing_error_message;
use crate::core::anthropic::sse::to_anthropic_format;
use crate::core::anthropic::tokens::TokenCounter;
use crate::models::{MessagesRequest, TokenCountRequest, TokenCountResponse};
use crate::providers::traits::ProviderError;
use crate::providers::Provider;

/// OpenAI Chat upstreams (NVIDIA NIM) cannot use Anthropic server tools without local handler.
const OPENAI_CHAT_UPSTREAM_IDS: &[&str] = &["nvidia_nim"];

pub type ProviderGetter = Arc<dyn Fn(&str) -> Box<dyn Provider> + Send + Sync>;

fn http_status_for_exception(err: &ProviderError) -> u16 {
    match err {
        ProviderError::InvalidRequest(_) => 400,
        ProviderError::Authentication(_) => 401,
        ProviderError::RateLimit(_) => 429,
        _ => 500,
    }
}

fn log_service_error(config: &Config, err: &dyn std::fmt::Display, context: &str) {
    if config.logging.error_tracebacks {
        tracing::error!("{}: {}", context, err);
    } else {
        tracing::error!("{} exc_type={}", context, std::any::type_name_of_val(err));
    }
}

fn require_non_empty_messages(messages: &[crate::models::Message]) -> Result<(), ProviderError> {
    if messages.is_empty() {
        return Err(ProviderError::invalid_request("messages cannot be empty"));
    }
    Ok(())
}

fn error_response(status: u16, type_field: &str, message: &str) -> Response {
    (
        axum::http::StatusCode::from_u16(status)
            .unwrap_or(axum::http::StatusCode::INTERNAL_SERVER_ERROR),
        Json(serde_json::json!({
            "error": {
                "type": type_field,
                "message": message
            }
        })),
    )
        .into_response()
}

pub struct ClaudeProxyService {
    config: Arc<Config>,
    provider_getter: ProviderGetter,
    model_router: ModelRouter,
}

impl ClaudeProxyService {
    pub fn new(config: Arc<Config>, provider_getter: ProviderGetter) -> Self {
        let model_router = ModelRouter::new((*config).clone());
        Self {
            config,
            provider_getter,
            model_router,
        }
    }

    pub fn config(&self) -> &Config {
        &self.config
    }

    pub fn provider_getter(&self) -> ProviderGetter {
        self.provider_getter.clone()
    }

    pub async fn create_message(&self, request_data: &MessagesRequest) -> Response {
        if let Err(e) = require_non_empty_messages(&request_data.messages) {
            return error_response(400, "invalid_request", &e.to_string());
        }

        let routed = self.model_router.resolve_messages_request(request_data);

        if OPENAI_CHAT_UPSTREAM_IDS.contains(&routed.resolved.provider_id.as_str()) {
            if let Some(err_msg) = openai_chat_upstream_server_tool_error(
                &routed.request,
                self.config.features.web_tools_enabled,
            ) {
                return error_response(400, "invalid_request", &err_msg);
            }
        }

        if self.config.features.web_tools_enabled
            && is_web_server_tool_request(&routed.request)
        {
            let input_tokens = TokenCounter::count_input_tokens(&routed.request) as i32;
            tracing::info!("Optimization: Handling Anthropic web server tool");

            let egress = Arc::new(WebFetchEgressPolicy::new(
                self.config.features.web_fetch_egress_allow_private,
                self.config.features.web_fetch_egress_allowed_schemes.clone(),
            ));

            let events = stream_web_server_tool_response(
                &routed.request,
                input_tokens,
                egress,
                self.config.logging.error_tracebacks,
            )
            .await;

            let sse_stream = tokio_stream::iter(events.into_iter().map(|data| {
                Ok::<_, std::convert::Infallible>(axum::response::sse::Event::default().data(data))
            }));

            return Sse::new(sse_stream)
                .keep_alive(axum::response::sse::KeepAlive::new())
                .into_response();
        }

        if let Some(optimized) = try_optimizations(&routed.request, &self.config) {
            return Json(serde_json::to_value(optimized).unwrap_or_default()).into_response();
        }
        tracing::debug!("No optimization matched, routing to provider");

        let provider = (self.provider_getter)(&routed.resolved.provider_id);

        let request_id = format!(
            "req_{}",
            &uuid::Uuid::new_v4().to_string().replace('-', "")[..12]
        );
        tracing::info!(
            "API_REQUEST: request_id={} model={} messages={}",
            request_id,
            routed.request.model,
            routed.request.messages.len(),
        );

        if routed.request.stream.unwrap_or(false) {
            let stream = match provider
                .create_streaming_chat_completion(&routed.request)
                .await
            {
                Ok(s) => s,
                Err(e) => {
                    log_service_error(&self.config, &e, "CREATE_MESSAGE_ERROR");
                    return error_response(
                        http_status_for_exception(&e) as u16,
                        "api_error",
                        &get_user_facing_error_message(&e.to_string()),
                    );
                }
            };

            let anthropic_stream = to_anthropic_format(stream.rx);

            let sse_stream =
                tokio_stream::wrappers::UnboundedReceiverStream::new(anthropic_stream.rx).map(
                    |value| {
                        let event_type = value.get("type").and_then(|t| t.as_str());
                        let mut event = axum::response::sse::Event::default();
                        if let Some(et) = event_type {
                            event = event.event(et.to_string());
                        }
                        Ok::<_, std::convert::Infallible>(
                            event.json_data(value).unwrap_or_default(),
                        )
                    },
                );

            return Sse::new(sse_stream)
                .keep_alive(axum::response::sse::KeepAlive::new())
                .into_response();
        }

        match provider.create_chat_completion(&routed.request).await {
            Ok(response) => Json(serde_json::to_value(response).unwrap_or_default()).into_response(),
            Err(e) => {
                log_service_error(&self.config, &e, "CREATE_MESSAGE_ERROR");
                error_response(
                    http_status_for_exception(&e) as u16,
                    "api_error",
                    &get_user_facing_error_message(&e.to_string()),
                )
            }
        }
    }

    pub fn count_tokens(
        &self,
        request_data: &TokenCountRequest,
    ) -> Result<TokenCountResponse, (u16, Value)> {
        if let Err(e) = require_non_empty_messages(&request_data.messages) {
            return Err((
                400,
                serde_json::json!({
                    "error": {
                        "type": "invalid_request",
                        "message": e.to_string()
                    }
                }),
            ));
        }

        let routed = self.model_router.resolve_token_count_request(request_data);

        let msg_req = MessagesRequest {
            model: routed.request.model.clone(),
            messages: routed.request.messages.clone(),
            original_model: None,
            resolved_provider_model: None,
            max_tokens: None,
            system: routed.request.system.clone(),
            stop_sequences: None,
            stream: None,
            temperature: None,
            top_p: None,
            top_k: None,
            metadata: None,
            tools: routed.request.tools.clone(),
            tool_choice: routed.request.tool_choice.clone(),
            thinking: routed.request.thinking.clone(),
            context_management: routed.request.context_management.clone(),
            output_config: routed.request.output_config.clone(),
            mcp_servers: routed.request.mcp_servers.clone(),
            extra_body: None,
            betas: routed.request.betas.clone(),
            extra: routed.request.extra.clone(),
        };

        let tokens = TokenCounter::count_input_tokens(&msg_req) as i32;
        Ok(TokenCountResponse { input_tokens: tokens })
    }
}
