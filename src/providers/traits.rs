//! Provider traits and common types

use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;
use thiserror::Error;

/// Provider error types
#[derive(Debug, Error)]
pub enum ProviderError {
    #[error("Network error: {0}")]
    Network(String),
    
    #[error("API error: {0}")]
    Api(String),
    
    #[error("Rate limit exceeded: {0}")]
    RateLimit(String),
    
    #[error("Authentication failed: {0}")]
    Authentication(String),
    
    #[error("Invalid request: {0}")]
    InvalidRequest(String),
    
    #[error("Internal error: {0}")]
    Internal(String),
}

impl ProviderError {
    pub fn network<S: Into<String>>(msg: S) -> Self {
        Self::Network(msg.into())
    }
    
    pub fn api<S: Into<String>>(msg: S) -> Self {
        Self::Api(msg.into())
    }
    
    pub fn rate_limit<S: Into<String>>(msg: S) -> Self {
        Self::RateLimit(msg.into())
    }
    
    pub fn authentication<S: Into<String>>(msg: S) -> Self {
        Self::Authentication(msg.into())
    }
    
    pub fn invalid_request<S: Into<String>>(msg: S) -> Self {
        Self::InvalidRequest(msg.into())
    }
    
    pub fn internal<S: Into<String>>(msg: S) -> Self {
        Self::Internal(msg.into())
    }
}

/// Provider trait
#[async_trait]
pub trait Provider: Send + Sync {
    /// Get the provider name
    fn name(&self) -> &str;
    
    /// Check if provider is configured
    fn is_configured(&self) -> bool;
    
    /// Create chat completion
    async fn create_chat_completion(
        &self,
        request: &crate::models::MessagesRequest,
    ) -> Result<crate::models::MessagesResponse, ProviderError>;
    
    /// Create streaming chat completion
    async fn create_streaming_chat_completion(
        &self,
        request: &crate::models::MessagesRequest,
    ) -> Result<ProviderStream, ProviderError>;
    
    /// Get available models
    async fn list_models(&self) -> Result<Vec<String>, ProviderError>;
    
    /// Count tokens for a request
    async fn count_tokens(&self, request: &crate::models::MessagesRequest) -> Result<HashMap<String, i32>, ProviderError>;
}

/// Stream implementation
pub struct ProviderStream {
    pub rx: tokio::sync::mpsc::UnboundedReceiver<Value>,
}

impl ProviderStream {
    pub fn new(rx: tokio::sync::mpsc::UnboundedReceiver<Value>) -> Self {
        Self { rx }
    }
}