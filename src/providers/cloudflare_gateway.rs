use async_trait::async_trait;
use futures::StreamExt;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use std::collections::HashMap;

use super::traits::{Provider, ProviderError, ProviderStream};

pub struct CloudflareGatewayProvider {
    api_key: Option<String>,
    base_url: Option<String>,
}

impl CloudflareGatewayProvider {
    pub fn new(api_key: Option<String>, base_url: Option<String>) -> Self {
        Self { api_key, base_url }
    }

    fn build_request_url(&self) -> String {
        self.base_url
            .as_deref()
            .unwrap_or("https://gateway.ai.cloudflare.com/v1")
            .to_string()
            + "/chat/completions"
    }

    fn build_request_headers(&self) -> HashMap<String, String> {
        let mut headers = HashMap::new();
        headers.insert("Content-Type".to_string(), "application/json".to_string());
        if let Some(api_key) = &self.api_key {
            headers.insert(
                "cf-aig-authorization".to_string(),
                format!("Bearer {}", api_key),
            );
        }
        headers
    }
}

#[async_trait]
impl Provider for CloudflareGatewayProvider {
    fn name(&self) -> &str {
        "cloudflare_gateway"
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
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        tokio::spawn(async move {
            let mut stream = response.bytes_stream();
            while let Some(chunk_result) = stream.next().await {
                match chunk_result {
                    Ok(chunk) => {
                        if let Ok(text) = String::from_utf8(chunk.to_vec()) {
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
        Ok(Vec::new())
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
