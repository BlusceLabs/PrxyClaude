use axum::{
    extract::{Path, State},
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::api::server::server::AppState;
use crate::api::web_tools::egress::WebFetchEgressPolicy;
use crate::api::web_tools::outbound::{run_web_fetch, run_web_search};
use crate::models::{MessagesRequest, TokenCountRequest};

#[derive(Serialize)]
pub struct ModelCapabilitiesResponse {
    pub supports_thinking: bool,
    pub supports_streaming: bool,
    pub supports_tool_use: bool,
}

#[derive(Deserialize)]
pub struct WebSearchRequest {
    pub query: String,
}

#[derive(Deserialize)]
pub struct WebFetchRequest {
    pub url: String,
}

pub async fn create_message(
    State(state): State<AppState>,
    axum::Json(request): axum::Json<MessagesRequest>,
) -> axum::response::Response {
    if let Err(e) = request.validate() {
        return (
            axum::http::StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": {"type": "invalid_request", "message": e}})),
        )
            .into_response();
    }

    state.service.create_message(&request).await
}

pub async fn probe_messages() -> impl IntoResponse {
    (
        axum::http::StatusCode::NO_CONTENT,
        [("Allow", "POST, HEAD, OPTIONS")],
    )
}

pub async fn count_tokens(
    State(state): State<AppState>,
    axum::Json(request): axum::Json<TokenCountRequest>,
) -> impl IntoResponse {
    match state.service.count_tokens(&request) {
        Ok(response) => Json(response).into_response(),
        Err((status, error)) => (
            axum::http::StatusCode::from_u16(status)
                .unwrap_or(axum::http::StatusCode::INTERNAL_SERVER_ERROR),
            Json(error),
        )
            .into_response(),
    }
}

pub async fn probe_count_tokens() -> impl IntoResponse {
    (
        axum::http::StatusCode::NO_CONTENT,
        [("Allow", "POST, HEAD, OPTIONS")],
    )
}

pub async fn root(State(state): State<AppState>) -> Json<Value> {
    let provider_name = &state.config.providers.default_provider;
    Json(serde_json::json!({
        "status": "ok",
        "provider": provider_name,
        "model": state.config.providers.model,
    }))
}

pub async fn probe_root() -> impl IntoResponse {
    (
        axum::http::StatusCode::NO_CONTENT,
        [("Allow", "GET, HEAD, OPTIONS")],
    )
}

pub async fn health() -> Json<Value> {
    Json(serde_json::json!({"status": "healthy"}))
}

pub async fn probe_health() -> impl IntoResponse {
    (
        axum::http::StatusCode::NO_CONTENT,
        [("Allow", "GET, HEAD, OPTIONS")],
    )
}

pub async fn models(State(state): State<AppState>) -> Json<Value> {
    let config = &state.config;
    let provider_name = &config.providers.default_provider;
    let mut models = Vec::new();

    let provider = (state.service.provider_getter())(provider_name);
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

    if models.is_empty() {
        models.push(serde_json::json!({
            "id": format!("{provider_name}/default"),
            "object": "model",
            "created": chrono::Utc::now().timestamp(),
            "owned_by": provider_name,
        }));
    }

    Json(serde_json::json!({
        "data": models,
        "object": "list",
    }))
}

pub async fn get_model(
    Path(model_id): Path<String>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let config = &state.config;
    let provider_name = &config.providers.default_provider;

    let provider = (state.service.provider_getter())(provider_name);
    match provider.list_models().await {
        Ok(ids) => {
            if ids.iter().any(|id| id == &model_id) {
                Json(serde_json::json!({
                    "id": model_id,
                    "object": "model",
                    "created": chrono::Utc::now().timestamp(),
                    "owned_by": provider_name,
                }))
                .into_response()
            } else {
                (
                    axum::http::StatusCode::NOT_FOUND,
                    Json(serde_json::json!({
                        "error": {
                            "type": "not_found",
                            "message": format!("Model '{}' not found", model_id)
                        }
                    })),
                )
                    .into_response()
            }
        }
        Err(e) => (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({
                "error": {
                    "type": "api_error",
                    "message": format!("Failed to list models: {}", e)
                }
            })),
        )
            .into_response(),
    }
}

pub async fn get_model_capabilities(
    Path(model_id): Path<String>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let config = &state.config;
    let provider_name = &config.providers.default_provider;

    let provider = (state.service.provider_getter())(provider_name);
    match provider.list_models().await {
        Ok(ids) => {
            if ids.iter().any(|id| id == &model_id) {
                Json(serde_json::json!({
                    "supports_thinking": config.providers.enable_model_thinking,
                    "supports_streaming": true,
                    "supports_tool_use": true,
                }))
                .into_response()
            } else {
                (
                    axum::http::StatusCode::NOT_FOUND,
                    Json(serde_json::json!({
                        "error": {
                            "type": "not_found",
                            "message": format!("Model '{}' not found", model_id)
                        }
                    })),
                )
                    .into_response()
            }
        }
        Err(e) => (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({
                "error": {
                    "type": "api_error",
                    "message": format!("Failed to list models: {}", e)
                }
            })),
        )
            .into_response(),
    }
}

pub async fn web_search(
    State(_state): State<AppState>,
    Json(request): Json<WebSearchRequest>,
) -> impl IntoResponse {
    if request.query.trim().is_empty() {
        return (
            axum::http::StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": {
                    "type": "invalid_request",
                    "message": "Query cannot be empty"
                }
            })),
        )
            .into_response();
    }

    match run_web_search(&request.query).await {
        Ok(results) => Json(serde_json::json!({
            "results": results,
            "query": request.query,
        }))
        .into_response(),
        Err(e) => (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({
                "error": {
                    "type": "api_error",
                    "message": format!("Web search failed: {}", e)
                }
            })),
        )
            .into_response(),
    }
}

pub async fn web_fetch(
    State(state): State<AppState>,
    Json(request): Json<WebFetchRequest>,
) -> impl IntoResponse {
    if request.url.trim().is_empty() {
        return (
            axum::http::StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": {
                    "type": "invalid_request",
                    "message": "URL cannot be empty"
                }
            })),
        )
            .into_response();
    }

    let egress = WebFetchEgressPolicy::new(
        state.config.features.web_fetch_egress_allow_private,
        state.config.features.web_fetch_egress_allowed_schemes.clone(),
    );

    match run_web_fetch(&request.url, &egress).await {
        Ok(result) => Json(serde_json::json!({
            "result": result,
        }))
        .into_response(),
        Err(e) => (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({
                "error": {
                    "type": "api_error",
                    "message": format!("Web fetch failed: {}", e)
                }
            })),
        )
            .into_response(),
    }
}
