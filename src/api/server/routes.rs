use axum::extract::State;
use axum::response::Json;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::models::MessagesRequest;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelResponse {
    pub created_at: String,
    pub display_name: String,
    pub id: String,
    #[serde(rename = "type")]
    pub type_field: String,
}

pub async fn models(
    State(state): State<crate::api::server::server::AppState>,
) -> Json<Value> {
    let mut models = Vec::new();
    
    // Add default provider models
    let provider_name = state.config.providers.default_provider.as_str();
    
    let (display_name, id) = match provider_name {
        "openai" => ("GPT-4".to_string(), "gpt-4".to_string()),
        "anthropic" => ("Claude 3".to_string(), "claude-3-sonnet-20240229".to_string()),
        "open_router" => ("Claude 3".to_string(), "anthropic/claude-3-sonnet".to_string()),
        "nvidia_nim" => ("Mixtral".to_string(), "nvidia/nim-mistral-8x7b-instruct".to_string()),
        _ => ("Unknown".to_string(), "unknown".to_string()),
    };
    
    models.push(ModelResponse {
        created_at: chrono::Utc::now().to_rfc3339(),
        display_name,
        id,
        type_field: "model".to_string(),
    });
    
    Json(serde_json::json!({
        "data": models,
        "object": "list",
    }))
}

pub async fn messages(
    State(_state): State<crate::api::server::server::AppState>,
    axum::Json(_request): axum::Json<MessagesRequest>,
) -> Json<Value> {
    // Placeholder implementation - would route to provider
    axum::Json(serde_json::json!({
        "error": "Not implemented yet"
    }))
}