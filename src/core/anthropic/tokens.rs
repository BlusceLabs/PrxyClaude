//! Token counting utilities

use crate::models::MessagesRequest;
use std::collections::HashMap;

/// Simple token counter (would be more sophisticated in production)
pub struct TokenCounter;

impl TokenCounter {
    /// Get token count for a message
    pub fn count_message_tokens(message: &crate::models::Message) -> usize {
        let mut tokens = 0;
        
        // Role tokens
        match message.role {
            crate::models::Role::User => tokens += 3,
            crate::models::Role::Assistant => tokens += 3,
            crate::models::Role::System => tokens += 3,
        }
        
        // Content tokens
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
        
        // Rough approximation: 4 characters per token
        tokens += content.chars().count() / 4;
        
        tokens
    }
    
    /// Get total input tokens for a request
    pub fn count_input_tokens(request: &MessagesRequest) -> usize {
        let mut total = 0;
        
        // System message tokens
        if let Some(system) = &request.system {
            let system_text = match system {
                crate::models::SystemContentOrString::System(sys) => &sys.text,
                crate::models::SystemContentOrString::String(s) => s,
            };
            total += system_text.chars().count() / 4;
        }
        
        // Message tokens
        for message in &request.messages {
            total += Self::count_message_tokens(message);
        }
        
        total
    }
    
    /// Estimate output tokens
    pub fn estimate_output_tokens(request: &MessagesRequest) -> usize {
        if let Some(max_tokens) = request.max_tokens {
            max_tokens as usize
        } else {
            // Default estimation
            1000
        }
    }
}

/// Get token count for a request
pub fn get_token_count(request: &MessagesRequest) -> HashMap<String, usize> {
    let mut result = HashMap::new();
    
    result.insert("input_tokens".to_string(), TokenCounter::count_input_tokens(request));
    result.insert("output_tokens".to_string(), TokenCounter::estimate_output_tokens(request));
    result.insert("total_tokens".to_string(), 
        TokenCounter::count_input_tokens(request) + TokenCounter::estimate_output_tokens(request));
    
    result
}