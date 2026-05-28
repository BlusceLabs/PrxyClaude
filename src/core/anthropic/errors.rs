use thiserror::Error;

/// Anthropic protocol errors
#[derive(Error, Debug)]
pub enum AnthropicError {
    #[error("API error: {0}")]
    ApiError(String),
    
    #[error("Rate limit exceeded")]
    RateLimitExceeded,
    
    #[error("Authentication failed")]
    AuthenticationFailed,
    
    #[error("Invalid request: {0}")]
    InvalidRequest(String),
    
    #[error("Provider error: {0}")]
    ProviderError(String),
    
    #[error("Timeout error")]
    Timeout,
    
    #[error("Connection error")]
    ConnectionError,
}

/// Format user error preview
pub fn format_user_error_preview(error: &str) -> String {
    format!("Error: {}", error)
}

/// Get user-facing error message
pub fn get_user_facing_error_message(error: &str) -> String {
    format!("An error occurred: {}", error)
}