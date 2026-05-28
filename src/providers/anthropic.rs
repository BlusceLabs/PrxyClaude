use async_trait::async_trait;
use futures::StreamExt;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use serde_json::Value;
use std::collections::HashMap;

use super::traits::{Provider, ProviderError, ProviderStream};

/// Anthropic provider implementation
pub struct AnthropicProvider {
    api_key: Option<String>,
    beta_features: Vec<String>,
}

impl AnthropicProvider {
    pub fn new(api_key: Option<String>, beta_features: Vec<String>) -> Self {
        Self {
            api_key,
            beta_features,
        }
    }
    
    fn build_request_url(&self) -> String {
        "https://api.anthropic.com/v1/messages".to_string()
    }
    
    fn build_request_headers(&self) -> HashMap<String, String> {
        let mut headers = HashMap::new();
        headers.insert("Content-Type".to_string(), "application/json".to_string());
        headers.insert("x-api-key".to_string(), self.api_key.clone().unwrap_or_default());
        headers.insert("anthropic-version".to_string(), "2023-06-01".to_string());
        
        if !self.beta_features.is_empty() {
            headers.insert("anthropic-beta".to_string(), self.beta_features.join(","));
        }
        
        headers
    }
}

#[async_trait]
impl Provider for AnthropicProvider {
    fn name(&self) -> &str {
        "anthropic"
    }
    
    fn is_configured(&self) -> bool {
        self.api_key.is_some()
    }
    
    async fn create_chat_completion(
        &self,
        request: &crate::models::MessagesRequest,
    ) -> Result<crate::models::MessagesResponse, ProviderError> {
        if !self.is_configured() {
            return Err(ProviderError::invalid_request("Provider not configured"));
        }
        
        let client = reqwest::Client::new();
        let url = self.build_request_url();
        let headers = self.build_request_headers();
        
        let response = client
            .post(&url)
            .headers({
                let mut hdrs = HeaderMap::new();
                for (key, value) in headers {
                    hdrs.insert(
                        HeaderName::from_bytes(key.as_bytes()).unwrap(),
                        HeaderValue::from_str(&value).unwrap(),
                    );
                }
                hdrs
            })
            .json(request)
            .send()
            .await
            .map_err(|e| ProviderError::network(e.to_string()))?;
        
        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(ProviderError::api(format!("HTTP {}: {}", status, error_text)));
        }
        
        let response: crate::models::MessagesResponse = response
            .json()
            .await
            .map_err(|e| ProviderError::api(e.to_string()))?;
        
        Ok(response)
    }
    
    async fn create_streaming_chat_completion(
        &self,
        request: &crate::models::MessagesRequest,
    ) -> Result<ProviderStream, ProviderError> {
        if !self.is_configured() {
            return Err(ProviderError::invalid_request("Provider not configured"));
        }
        
        let client = reqwest::Client::new();
        let url = self.build_request_url();
        let headers = self.build_request_headers();
        
        let mut streaming_request = request.clone();
        streaming_request.stream = Some(true);
        
        let response = client
            .post(&url)
            .headers({
                let mut hdrs = HeaderMap::new();
                for (key, value) in headers {
                    hdrs.insert(
                        HeaderName::from_bytes(key.as_bytes()).unwrap(),
                        HeaderValue::from_str(&value).unwrap(),
                    );
                }
                hdrs
            })
            .json(&streaming_request)
            .send()
            .await
            .map_err(|e| ProviderError::network(e.to_string()))?;
        
        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(ProviderError::api(format!("HTTP {}: {}", status, error_text)));
        }
        
        // Create a stream from the response
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        tokio::spawn(async move {
            let mut stream = response.bytes_stream();
            while let Some(chunk_result) = stream.next().await {
                match chunk_result {
                    Ok(chunk) => {
                        if let Ok(text) = String::from_utf8(chunk.to_vec()) {
                            // Parse SSE events
                            for line in text.lines() {
                                if line.starts_with("data: ") {
                                    let json_str = &line[6..];
                                    if json_str != "[DONE]" {
                                        if let Ok(value) = serde_json::from_str(json_str) {
                                            let _ = tx.send(value);
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => {
                        let _ = tx.send(serde_json::json!({
                            "type": "error",
                            "error": {
                                "type": "stream_error",
                                "message": e.to_string()
                            }
                        }));
                        break;
                    }
                }
            }
        });
        
        Ok(ProviderStream::new(rx))
    }
    
    async fn list_models(&self) -> Result<Vec<String>, ProviderError> {
        let client = reqwest::Client::new();
        let url = "https://api.anthropic.com/v1/models".to_string();
        let headers = self.build_request_headers();
        
        let response = client
            .get(&url)
            .headers({
                let mut hdrs = HeaderMap::new();
                for (key, value) in headers {
                    hdrs.insert(
                        HeaderName::from_bytes(key.as_bytes()).unwrap(),
                        HeaderValue::from_str(&value).unwrap(),
                    );
                }
                hdrs
            })
            .send()
            .await
            .map_err(|e| ProviderError::network(e.to_string()))?;
        
        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(ProviderError::api(format!("HTTP {}: {}", status, error_text)));
        }
        
        let models_response: ModelsResponse = response
            .json()
            .await
            .map_err(|e| ProviderError::api(e.to_string()))?;
        
        Ok(models_response.data.into_iter().map(|m| m.id).collect())
    }
    
    async fn count_tokens(&self, request: &crate::models::MessagesRequest) -> Result<HashMap<String, i32>, ProviderError> {
        let client = reqwest::Client::new();
        let url = "https://api.anthropic.com/v1/messages/count_tokens".to_string();
        let headers = self.build_request_headers();
        
        let token_request = TokenCountRequest {
            model: request.model.clone(),
            messages: request.messages.clone(),
            system: request.system.clone(),
            tools: request.tools.clone(),
            thinking: request.thinking.clone(),
            tool_choice: request.tool_choice.clone(),
            context_management: request.context_management.clone(),
            output_config: request.output_config.clone(),
            mcp_servers: request.mcp_servers.clone(),
            betas: request.betas.clone(),
            extra: HashMap::new(),
        };
        
        let response = client
            .post(&url)
            .headers({
                let mut hdrs = HeaderMap::new();
                for (key, value) in headers {
                    hdrs.insert(
                        HeaderName::from_bytes(key.as_bytes()).unwrap(),
                        HeaderValue::from_str(&value).unwrap(),
                    );
                }
                hdrs
            })
            .json(&token_request)
            .send()
            .await
            .map_err(|e| ProviderError::network(e.to_string()))?;
        
        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(ProviderError::api(format!("HTTP {}: {}", status, error_text)));
        }
        
        let token_response: crate::models::TokenCountResponse = response
            .json()
            .await
            .map_err(|e| ProviderError::api(e.to_string()))?;
        
        Ok(HashMap::from([
            ("input_tokens".to_string(), token_response.input_tokens),
        ]))
    }
}

// Response types
#[derive(serde::Deserialize)]
struct ModelsResponse {
    data: Vec<AnthropicModel>,
}

#[derive(serde::Deserialize)]
#[allow(dead_code)]
struct AnthropicModel {
    id: String,
    created_at: String,
    display_name: String,
    type_field: String,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct TokenCountRequest {
    model: String,
    messages: Vec<crate::models::Message>,
    system: Option<crate::models::SystemContentOrString>,
    tools: Option<Vec<crate::models::Tool>>,
    thinking: Option<crate::models::ThinkingConfig>,
    tool_choice: Option<Value>,
    context_management: Option<HashMap<String, Value>>,
    output_config: Option<HashMap<String, Value>>,
    mcp_servers: Option<Vec<HashMap<String, Value>>>,
    betas: Option<Vec<String>>,
    extra: HashMap<String, Value>,
}