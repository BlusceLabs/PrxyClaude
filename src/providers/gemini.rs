use async_trait::async_trait;
use futures::StreamExt;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use serde_json::Value;
use std::collections::HashMap;

use super::traits::{Provider, ProviderError, ProviderStream};

const GEMINI_DEFAULT_BASE: &str =
    "https://generativelanguage.googleapis.com/v1beta/openai";
const GEMINI_MODEL_PREFIX: &str = "models/";

pub struct GeminiProvider {
    api_key: Option<String>,
    base_url: Option<String>,
}

impl GeminiProvider {
    pub fn new(api_key: Option<String>, base_url: Option<String>) -> Self {
        Self { api_key, base_url }
    }

    fn build_request_url(&self) -> String {
        self.base_url
            .as_deref()
            .unwrap_or(GEMINI_DEFAULT_BASE)
            .to_string()
            + "/chat/completions"
    }

    fn build_request_headers(&self) -> HashMap<String, String> {
        let mut headers = HashMap::new();
        headers.insert("Content-Type".to_string(), "application/json".to_string());
        if let Some(api_key) = &self.api_key {
            headers.insert("Authorization".to_string(), format!("Bearer {}", api_key));
        }
        headers
    }
}

#[async_trait]
impl Provider for GeminiProvider {
    fn name(&self) -> &str {
        "gemini"
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
        let openai_request = self.convert_to_openai_format(request)?;
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
            .json(&openai_request)
            .send()
            .await
            .map_err(|e| ProviderError::network(e.to_string()))?;
        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(ProviderError::api(format!("HTTP {}: {}", status, error_text)));
        }
        let openai_response: OpenAIResponse = response
            .json()
            .await
            .map_err(|e| ProviderError::api(e.to_string()))?;
        self.convert_from_openai_format(&openai_response)
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
        let mut openai_request = self.convert_to_openai_format(request)?;
        openai_request
            .as_object_mut()
            .unwrap()
            .insert("stream".to_string(), Value::Bool(true));
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
            .json(&openai_request)
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
                            if let Ok(value) = serde_json::from_str(&text) {
                                let _ = tx.send(value);
                            }
                        }
                    }
                    Err(e) => {
                        let _ = tx.send(serde_json::json!({
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
        let base = self.base_url.as_deref().unwrap_or(GEMINI_DEFAULT_BASE);
        let url = format!("{}/models", base);
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
            return Ok(Vec::new());
        }
        let models_response: serde_json::Value = response
            .json()
            .await
            .map_err(|e| ProviderError::api(e.to_string()))?;
        let models = models_response
            .get("data")
            .and_then(|d| d.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|m| {
                        m.get("id")
                            .and_then(|n| n.as_str())
                            .map(|s| s.strip_prefix(GEMINI_MODEL_PREFIX).unwrap_or(s).to_string())
                    })
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

impl GeminiProvider {
    fn convert_to_openai_format(
        &self,
        request: &crate::models::MessagesRequest,
    ) -> Result<Value, ProviderError> {
        let mut body = HashMap::new();
        body.insert("model".to_string(), Value::String(request.model.clone()));
        if let Some(max_tokens) = request.max_tokens {
            body.insert("max_tokens".to_string(), Value::Number(max_tokens.into()));
        }
        if let Some(temperature) = request.temperature {
            body.insert("temperature".to_string(), Value::from(temperature));
        }
        if let Some(top_p) = request.top_p {
            body.insert("top_p".to_string(), Value::from(top_p));
        }
        if let Some(stream) = request.stream {
            body.insert("stream".to_string(), Value::Bool(stream));
        }
        let mut messages = Vec::new();
        for message in &request.messages {
            let role = match message.role {
                crate::models::Role::User => "user",
                crate::models::Role::Assistant => "assistant",
                crate::models::Role::System => "system",
            };
            let content = match &message.content {
                crate::models::ContentOrBlocks::String(s) => s.clone(),
                crate::models::ContentOrBlocks::Blocks(blocks) => {
                    let mut text = String::new();
                    for block in blocks {
                        if let Some(text_part) = block.as_text() {
                            text.push_str(text_part);
                        }
                    }
                    text
                }
            };
            let mut message_obj = HashMap::new();
            message_obj.insert("role".to_string(), Value::String(role.to_string()));
            message_obj
                .insert("content".to_string(), Value::String(content));
            messages.push(serde_json::json!(message_obj));
        }
        body.insert("messages".to_string(), Value::Array(messages));
        Ok(serde_json::json!(body))
    }

    fn convert_from_openai_format(
        &self,
        response: &OpenAIResponse,
    ) -> Result<crate::models::MessagesResponse, ProviderError> {
        let usage = response.usage.clone().unwrap_or_else(|| crate::models::Usage {
            input_tokens: 0,
            output_tokens: 0,
            cache_creation_input_tokens: 0,
            cache_read_input_tokens: 0,
        });
        let mut content = Vec::new();
        for choice in &response.choices {
            if let Some(message) = &choice.message {
                let mut content_obj = HashMap::new();
                content_obj.insert("type".to_string(), Value::String("text".to_string()));
                content_obj.insert(
                    "text".to_string(),
                    Value::String(message.content.clone().unwrap_or_default()),
                );
                content.push(serde_json::json!(content_obj));
            }
        }
        Ok(crate::models::MessagesResponse::new(
            response.id.clone(),
            response.model.clone(),
            content,
            usage,
        ))
    }
}

#[derive(serde::Deserialize)]
struct OpenAIResponse {
    id: String,
    model: String,
    choices: Vec<OpenAIChoice>,
    usage: Option<crate::models::Usage>,
}

#[derive(serde::Deserialize)]
#[allow(dead_code)]
struct OpenAIChoice {
    index: i32,
    message: Option<OpenAIMessage>,
    finish_reason: Option<String>,
}

#[derive(serde::Deserialize)]
#[allow(dead_code)]
struct OpenAIMessage {
    role: String,
    content: Option<String>,
}
