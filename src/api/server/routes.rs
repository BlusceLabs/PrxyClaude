use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response, Sse},
    Json,
};
use serde_json::Value;
use std::convert::Infallible;
use tokio_stream::StreamExt;

use crate::config::Config;
use crate::models::MessagesRequest;
use crate::providers::{
    AnthropicProvider, DeepSeekProvider, FireworksProvider, GeminiProvider, KimiProvider,
    LlamaCppProvider, LmstudioProvider, NvidiaNimProvider, OllamaProvider, OpenAIProvider,
    OpenRouterProvider, Provider, SiliconFlowProvider, ZAIProvider,
};

pub async fn models(State(state): State<crate::api::server::server::AppState>) -> Json<Value> {
    let config = &state.config;
    let provider_name = &config.providers.default_provider;
    let mut models = Vec::new();

    match resolve_provider_by_name(provider_name, config) {
        Ok(provider) => {
            if let Ok(ids) = provider.list_models().await {
                for id in ids {
                    models.push(serde_json::json!({
                        "id": id,
                        "object": "model",
                        "created": chrono::Utc::now().timestamp(),
                        "owned_by": provider_name,
                    }));
                }
            }
        }
        Err(_) => {
            models.push(serde_json::json!({
                "id": provider_name.to_string() + "/default",
                "object": "model",
                "created": chrono::Utc::now().timestamp(),
                "owned_by": provider_name,
            }));
        }
    }

    Json(serde_json::json!({
        "data": models,
        "object": "list",
    }))
}

pub async fn messages(
    State(state): State<crate::api::server::server::AppState>,
    axum::Json(request): axum::Json<MessagesRequest>,
) -> Response {
    if let Err(e) = request.validate() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": {"type": "invalid_request", "message": e}})),
        )
            .into_response();
    }

    let provider = match resolve_provider(&state.config, &request.model) {
        Ok(p) => p,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": {"type": "invalid_request", "message": e}})),
            )
                .into_response();
        }
    };

    if request.stream.unwrap_or(false) {
        match provider.create_streaming_chat_completion(&request).await {
            Ok(stream) => {
                let sse_stream = tokio_stream::wrappers::UnboundedReceiverStream::new(stream.rx)
                    .map(|value| {
                        let event = match value.get("event").and_then(|e| e.as_str()) {
                            Some(event_type) => {
                                axum::response::sse::Event::default()
                                    .event(event_type.to_string())
                                    .json_data(value).unwrap_or_default()
                            }
                            None => {
                                axum::response::sse::Event::default()
                                    .json_data(value).unwrap_or_default()
                            }
                        };
                        Ok::<_, Infallible>(event)
                    });
                Sse::new(sse_stream)
                    .keep_alive(axum::response::sse::KeepAlive::new())
                    .into_response()
            }
            Err(e) => {
                let error_body = error_sse_body(&e.to_string());
                (StatusCode::INTERNAL_SERVER_ERROR, Json(error_body)).into_response()
            }
        }
    } else {
        match provider.create_chat_completion(&request).await {
            Ok(response) => Json(serde_json::to_value(response).unwrap_or_default()).into_response(),
            Err(e) => {
                let error_body = serde_json::json!({
                    "error": {
                        "type": "api_error",
                        "message": e.to_string()
                    }
                });
                (StatusCode::INTERNAL_SERVER_ERROR, Json(error_body)).into_response()
            }
        }
    }
}

fn error_sse_body(message: &str) -> Value {
    serde_json::json!({
        "error": {
            "type": "api_error",
            "message": message
        }
    })
}

fn resolve_provider(config: &Config, model: &str) -> Result<Box<dyn Provider>, String> {
    let provider_name = match model.split_once('/') {
        Some((p, _)) => p,
        None => config.providers.default_provider.as_str(),
    };
    resolve_provider_by_name(provider_name, config)
}

fn resolve_provider_by_name(name: &str, config: &Config) -> Result<Box<dyn Provider>, String> {
    match name {
        "openai" => Ok(Box::new(OpenAIProvider::new(
            config.providers.openai.api_key.clone(),
            config.providers.openai.base_url.clone(),
            config.providers.openai.organization.clone(),
        ))),
        "anthropic" => Ok(Box::new(AnthropicProvider::new(
            config.providers.anthropic.api_key.clone(),
            config.providers.anthropic.beta_features.clone(),
        ))),
        "open_router" => Ok(Box::new(OpenRouterProvider::new(
            config.providers.open_router.api_key.clone(),
            config.providers.open_router.base_url.clone(),
        ))),
        "nvidia_nim" => Ok(Box::new(NvidiaNimProvider::new(
            config.providers.nvidia_nim.base_url.clone(),
            config.providers.nvidia_nim.model_id.clone(),
        ))),
        "deepseek" => Ok(Box::new(DeepSeekProvider::new(
            config.providers.deepseek.api_key.clone(),
        ))),
        "kimi" => Ok(Box::new(KimiProvider::new(
            config.providers.kimi.api_key.clone(),
        ))),
        "llamacpp" => Ok(Box::new(LlamaCppProvider::new(
            config.providers.llamacpp.base_url.clone(),
        ))),
        "lmstudio" => Ok(Box::new(LmstudioProvider::new(
            config.providers.lmstudio.base_url.clone(),
        ))),
        "fireworks" => Ok(Box::new(FireworksProvider::new(
            config.providers.fireworks.api_key.clone(),
            config.providers.fireworks.base_url.clone(),
        ))),
        "siliconflow" => Ok(Box::new(SiliconFlowProvider::new(
            config.providers.siliconflow.api_key.clone(),
            config.providers.siliconflow.base_url.clone(),
        ))),
        "z_ai" => Ok(Box::new(ZAIProvider::new(
            config.providers.z_ai.api_key.clone(),
            config.providers.z_ai.base_url.clone(),
        ))),
        "gemini" => Ok(Box::new(GeminiProvider::new(
            config.providers.gemini.api_key.clone(),
            config.providers.gemini.base_url.clone(),
        ))),
        "ollama" => Ok(Box::new(OllamaProvider::new(
            config.providers.ollama.api_key.clone(),
            config.providers.ollama.base_url.clone(),
        ))),
        _ => Err(format!("Unknown provider: {name}")),
    }
}
