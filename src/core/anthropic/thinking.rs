use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Thinking error types
#[derive(Debug, Error)]
pub enum ThinkingError {
    #[error("Invalid thinking content: {0}")]
    InvalidContent(String),
    
    #[error("Missing signature: {0}")]
    MissingSignature(String),
    
    #[error("Invalid signature: {0}")]
    InvalidSignature(String),
}

/// Content chunk for thinking
#[derive(Debug, Clone)]
pub struct ContentChunk {
    pub content_type: ContentType,
    pub text: String,
    pub timestamp: DateTime<Utc>,
}

/// Content type enumeration
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ContentType {
    Text,
    Thinking,
    ToolUse,
    ToolResult,
}

/// Think tag parser for parsing thinking content
pub struct ThinkTagParser;

impl ThinkTagParser {
    /// Parse thinking content from text
    pub fn parse_thinking(text: &str) -> Vec<ContentChunk> {
        let mut chunks = Vec::new();
        
        // Simple implementation - in practice this would be more sophisticated
        if text.contains("<thinking>") {
            if let Some(start) = text.find("<thinking>") {
                if let Some(end) = text.find("</thinking>") {
                    let thinking_content = &text[start + 10..end];
                    chunks.push(ContentChunk {
                        content_type: ContentType::Thinking,
                        text: thinking_content.to_string(),
                        timestamp: Utc::now(),
                    });
                }
            }
        }
        
        // Add remaining text as regular content
        if chunks.is_empty() {
            chunks.push(ContentChunk {
                content_type: ContentType::Text,
                text: text.to_string(),
                timestamp: Utc::now(),
            });
        }
        
        chunks
    }
}