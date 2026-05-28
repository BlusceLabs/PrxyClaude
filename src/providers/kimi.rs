use async_trait::async_trait;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use serde_json::Value;
use std::collections::HashMap;
use super::traits::{Provider, ProviderError, ProviderStream};

/// Kimi provider implementation
pub struct KimiProvider {
    api_key: Option<String>,
}

impl KimiProvider {
    pub fn new(api_key: Option<String>) -> Self {
        Self { api_key }
    }
    
    fn build_request_url(&self) -> String {
        "https://api.moonshot.cn/v1/chat/completions".to_string()
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
impl Provider for KimiProvider {
    fn name(&self) -> &str {
        "kimi"
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
        
        let mut hdrs = HeaderMap::new();
        for (key, value) in headers {
            hdrs.insert(
                HeaderName::from_bytes(key.as_bytes()).unwrap(),
                HeaderValue::from_str(&value).unwrap(),
            );
        }
        
        let response = client
            .post(&url)
            .headers(hdrs)
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
        _request: &crate::models::MessagesRequest,
    ) -> Result<ProviderStream, ProviderError> {
        Err(ProviderError::invalid_request("Provider not configured"))
    }
    
    async fn list_models(&self) -> Result<Vec<String>, ProviderError> {
        Ok(vec![
            "moonshot-v1-8k".to_string(),
            "moonshot-v1-32k".to_string(),
            "moonshot-v1-128k".to_string(),
        ])
    }
    
    async fn count_tokens(&self, request: &crate::models::MessagesRequest) -> Result<HashMap<String, i32>, ProviderError> {
        let input_tokens = crate::core::anthropic::tokens::TokenCounter::count_input_tokens(request);
        let output_tokens = crate::core::anthropic::tokens::TokenCounter::estimate_output_tokens(request);
        
        Ok(HashMap::from([
            ("input_tokens".to_string(), input_tokens as i32),
            ("output_tokens".to_string(), output_tokens as i32),
        ]))
    }
}

impl KimiProvider {
    fn convert_to_openai_format(&self, request: &crate::models::MessagesRequest) -> Result<Value, ProviderError> {
        let mut body = HashMap::new();
        
        body.insert("model".to_string(), Value::String(request.model.clone()));
        
        if let Some(max_tokens) = request.max_tokens {
            body.insert("max_tokens".to_string(), Value::Number(max_tokens.into()));
        }
        
        if let Some(temperature) = request.temperature {
            body.insert("temperature".to_string(), Value::from(temperature));
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
            message_obj.insert("content".to_string(), Value::String(content));
            
            messages.push(serde_json::json!(message_obj));
        }
        
        body.insert("messages".to_string(), Value::Array(messages));
        
        Ok(serde_json::json!(body))
    }
    
    fn convert_from_openai_format(&self, response: &OpenAIResponse) -> Result<crate::models::MessagesResponse, ProviderError> {
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
                content_obj.insert("text".to_string(), Value::String(message.content.clone().unwrap_or_default()));
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