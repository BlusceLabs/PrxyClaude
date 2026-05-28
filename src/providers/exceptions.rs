use serde_json::Value;
use std::fmt;

#[derive(Debug, Clone)]
pub struct ProviderError {
    pub message: String,
    pub status_code: u16,
    pub error_type: String,
    pub raw_error: Option<String>,
}

impl ProviderError {
    pub fn new(message: &str, status_code: u16, error_type: &str) -> Self {
        Self {
            message: message.to_string(),
            status_code,
            error_type: error_type.to_string(),
            raw_error: None,
        }
    }

    pub fn authentication(message: &str) -> Self {
        Self {
            message: message.to_string(),
            status_code: 401,
            error_type: "authentication_error".to_string(),
            raw_error: None,
        }
    }

    pub fn invalid_request(message: &str) -> Self {
        Self {
            message: message.to_string(),
            status_code: 400,
            error_type: "invalid_request_error".to_string(),
            raw_error: None,
        }
    }

    pub fn rate_limit(message: &str) -> Self {
        Self {
            message: message.to_string(),
            status_code: 429,
            error_type: "rate_limit_error".to_string(),
            raw_error: None,
        }
    }

    pub fn overloaded(message: &str) -> Self {
        Self {
            message: message.to_string(),
            status_code: 529,
            error_type: "overloaded_error".to_string(),
            raw_error: None,
        }
    }

    pub fn api_error(message: &str, status_code: u16) -> Self {
        Self {
            message: message.to_string(),
            status_code,
            error_type: "api_error".to_string(),
            raw_error: None,
        }
    }

    pub fn service_unavailable(message: &str) -> Self {
        Self {
            message: message.to_string(),
            status_code: 503,
            error_type: "api_error".to_string(),
            raw_error: None,
        }
    }

    pub fn model_list_error(message: &str) -> Self {
        Self {
            message: message.to_string(),
            status_code: 503,
            error_type: "api_error".to_string(),
            raw_error: None,
        }
    }

    pub fn to_anthropic_format(&self) -> Value {
        serde_json::json!({
            "type": "error",
            "error": {
                "type": self.error_type,
                "message": self.message,
            },
        })
    }
}

impl fmt::Display for ProviderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {}: {}", self.status_code, self.error_type, self.message)
    }
}

impl std::error::Error for ProviderError {}
