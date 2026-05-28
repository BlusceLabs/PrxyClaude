use thiserror::Error;

/// Provider stream error
#[derive(Debug, Error)]
pub enum ProviderStreamError {
    #[error("Stream error: {0}")]
    StreamError(String),
    
    #[error("Parse error: {0}")]
    ParseError(String),
    
    #[error("Network error: {0}")]
    NetworkError(String),
    
    #[error("Invalid response: {0}")]
    InvalidResponse(String),
}

impl ProviderStreamError {
    pub fn stream_error<S: Into<String>>(msg: S) -> Self {
        Self::StreamError(msg.into())
    }
    
    pub fn parse_error<S: Into<String>>(msg: S) -> Self {
        Self::ParseError(msg.into())
    }
    
    pub fn network_error<S: Into<String>>(msg: S) -> Self {
        Self::NetworkError(msg.into())
    }
    
    pub fn invalid_response<S: Into<String>>(msg: S) -> Self {
        Self::InvalidResponse(msg.into())
    }
}

