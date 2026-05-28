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
pub fn format_user_error_preview(error: &str, max_len: usize) -> String {
    if error.len() > max_len {
        format!("{}...", &error[..max_len])
    } else {
        error.to_string()
    }
}

/// Get user-facing error message
pub fn get_user_facing_error_message(error: &str) -> String {
    if error.is_empty() {
        "Provider request failed unexpectedly.".to_string()
    } else {
        error.to_string()
    }
}