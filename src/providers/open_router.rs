use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};

use super::traits::{Provider, ProviderError, ProviderStream};

/// OpenRouter provider implementation
pub struct OpenRouterProvider {
    api_key: Option<String>,
    base_url: String,
    referer: Option<String>,
}

impl OpenRouterProvider {
    pub fn new(api_key: Option<String>, base_url: String, referer: Option<String>) -> Self {
        Self {
            api_key,
            base_url,
            referer,
        }
    }
    
    fn build_request_url(&self) -> String {
        format!("{}{}", self.base_url, "/chat/completions")
    }
    
    fn build_request_headers(&self) -> HashMap<String, String> {
        let mut headers = HashMap::new();
        headers.insert("Content-Type".to_string(), "application/json".to_string());
        if let Some(referer) = &self.referer {
            headers.insert("HTTP-Referer".to_string(), referer.clone());
        }
        headers.insert("X-Title".to_string(), "PxyClaude".to_string());
        
        if let Some(api_key) = &self.api_key {
            headers.insert("Authorization".to_string(), format!("Bearer {}", api_key));
        }
        
        headers
    }
}

#[async_trait]
impl Provider for OpenRouterProvider {
    fn name(&self) -> &str {
        "open_router"
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
        
        // Convert Anthropic request to OpenAI format
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
        openai_request.as_object_mut().unwrap().insert("stream".to_string(), Value::Bool(true));
        
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
        
        Ok(crate::core::anthropic::sse::parse_sse_response(response))
    }
    
    async fn list_models(&self) -> Result<Vec<String>, ProviderError> {
        let client = reqwest::Client::new();
        let url = format!("{}{}", self.base_url, "/models");
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
        // Simple token counting - in production, use OpenAI's tiktoken library
        let input_tokens = crate::core::anthropic::tokens::TokenCounter::count_input_tokens(request);
        let output_tokens = crate::core::anthropic::tokens::TokenCounter::estimate_output_tokens(request);
        
        Ok(HashMap::from([
            ("input_tokens".to_string(), input_tokens as i32),
            ("output_tokens".to_string(), output_tokens as i32),
        ]))
    }
}

impl OpenRouterProvider {
    fn convert_to_openai_format(&self, request: &crate::models::MessagesRequest) -> Result<Value, ProviderError> {
        let mut body = serde_json::json!({});
        
        body.as_object_mut().unwrap().insert("model".to_string(), Value::String(request.model.clone()));
        
        if let Some(max_tokens) = request.max_tokens {
            body.as_object_mut().unwrap().insert("max_tokens".to_string(), Value::Number(max_tokens.into()));
        }
        
        if let Some(temperature) = request.temperature {
            body.as_object_mut().unwrap().insert("temperature".to_string(), Value::from(temperature));
        }
        
        if let Some(top_p) = request.top_p {
            body.as_object_mut().unwrap().insert("top_p".to_string(), Value::from(top_p));
        }
        
        if let Some(stream) = request.stream {
            body.as_object_mut().unwrap().insert("stream".to_string(), Value::Bool(stream));
        }
        
        // Convert messages
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
            
            let mut message_obj = serde_json::json!({});
            message_obj.as_object_mut().unwrap().insert("role".to_string(), Value::String(role.to_string()));
            message_obj.as_object_mut().unwrap().insert("content".to_string(), Value::String(content));
            
            messages.push(message_obj);
        }
        
        body.as_object_mut().unwrap().insert("messages".to_string(), Value::Array(messages));
        
        Ok(body)
    }
    
    fn convert_from_openai_format(&self, response: &OpenAIResponse) -> Result<crate::models::MessagesResponse, ProviderError> {
        let usage = response.usage.clone().unwrap_or_else(|| crate::models::Usage {
            input_tokens: 0,
            output_tokens: 0,
            cache_creation_input_tokens: 0,
            cache_read_input_tokens: 0,
        });
        
        let mut content = Vec::new();
        let choices = &response.choices;
        for choice in choices {
            if let Some(message) = &choice.message {
                let mut content_obj = serde_json::json!({});
                content_obj.as_object_mut().unwrap().insert("type".to_string(), Value::String("text".to_string()));
                content_obj.as_object_mut().unwrap().insert("text".to_string(), Value::String(message.content.clone().unwrap_or_default()));
                content.push(content_obj);
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

// Response types
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

#[derive(serde::Deserialize)]
struct ModelsResponse {
    data: Vec<OpenRouterModel>,
}

#[derive(serde::Deserialize)]
#[allow(dead_code)]
struct OpenRouterModel {
    id: String,
    name: String,
    created_at: String,
    description: Option<String>,
}