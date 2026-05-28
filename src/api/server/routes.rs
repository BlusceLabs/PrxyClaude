use axum::{
    extract::State,
    response::IntoResponse,
    Json,
};
use serde_json::Value;

use crate::api::server::server::AppState;
use crate::models::MessagesRequest;

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

pub async fn messages(
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
