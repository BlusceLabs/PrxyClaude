use async_trait::async_trait;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use std::collections::HashMap;

use super::traits::{Provider, ProviderError, ProviderStream};

const OLLAMA_DEFAULT_BASE: &str = "http://localhost:11434";

pub struct OllamaProvider {
    api_key: Option<String>,
    base_url: Option<String>,
}

impl OllamaProvider {
    pub fn new(api_key: Option<String>, base_url: Option<String>) -> Self {
        Self { api_key, base_url }
    }

    fn base(&self) -> String {
        self.base_url
            .as_deref()
            .unwrap_or(OLLAMA_DEFAULT_BASE)
            .to_string()
    }

    fn build_request_headers(&self) -> HashMap<String, String> {
        let mut headers = HashMap::new();
        headers.insert("Content-Type".to_string(), "application/json".to_string());
        headers.insert(
            "x-api-key".to_string(),
            self.api_key.clone().unwrap_or_else(|| "ollama".to_string()),
        );
        headers
    }
}

#[async_trait]
impl Provider for OllamaProvider {
    fn name(&self) -> &str {
        "ollama"
    }

    fn is_configured(&self) -> bool {
        true
    }

    async fn create_chat_completion(
        &self,
        request: &crate::models::MessagesRequest,
    ) -> Result<crate::models::MessagesResponse, ProviderError> {
        let client = reqwest::Client::new();
        let url = format!("{}/v1/messages", self.base());
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
        let messages_response: crate::models::MessagesResponse = response
            .json()
            .await
            .map_err(|e| ProviderError::api(e.to_string()))?;
        Ok(messages_response)
    }

    async fn create_streaming_chat_completion(
        &self,
        request: &crate::models::MessagesRequest,
    ) -> Result<ProviderStream, ProviderError> {
        let client = reqwest::Client::new();
        let url = format!("{}/v1/messages", self.base());
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
        Ok(crate::core::anthropic::sse::parse_sse_response(response))
    }

    async fn list_models(&self) -> Result<Vec<String>, ProviderError> {
        let client = reqwest::Client::new();
        let url = format!("{}/api/tags", self.base());
        let response = client
            .get(&url)
            .send()
            .await
            .map_err(|e| ProviderError::network(e.to_string()))?;
        if !response.status().is_success() {
            return Ok(Vec::new());
        }
        let tags_response: serde_json::Value = response
            .json()
            .await
            .map_err(|e| ProviderError::api(e.to_string()))?;
        let models = tags_response
            .get("models")
            .and_then(|m| m.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|m| m.get("name").and_then(|n| n.as_str().map(String::from)))
                    .collect()
            })
            .unwrap_or_default();
        Ok(models)
    }

    async fn count_tokens(
        &self,
        request: &crate::models::MessagesRequest,
    ) -> Result<HashMap<String, i32>, ProviderError> {
        let input_tokens =
            crate::core::anthropic::tokens::TokenCounter::count_input_tokens(request);
        let output_tokens =
            crate::core::anthropic::tokens::TokenCounter::estimate_output_tokens(request);
        Ok(HashMap::from([
            ("input_tokens".to_string(), input_tokens as i32),
            ("output_tokens".to_string(), output_tokens as i32),
        ]))
    }
}
