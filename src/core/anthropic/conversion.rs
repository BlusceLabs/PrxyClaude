use serde_json::Value;
use std::collections::HashMap;

use crate::models::{ContentOrBlocks, ContentBlock, MessagesResponse, Role, Usage};

/// Convert Anthropic messages to OpenAI format
pub fn convert_anthropic_to_openai(messages: Vec<crate::models::Message>) -> Value {
    let mut openai_messages = Vec::new();
    
    for message in messages {
        let role = match message.role {
            Role::User => "user",
            Role::Assistant => "assistant",
            Role::System => "system",
        };
        
        let content = match message.content {
            ContentOrBlocks::String(s) => vec![serde_json::json!({
                "type": "text",
                "text": s,
            })],
            ContentOrBlocks::Blocks(blocks) => blocks.into_iter().map(|block| {
                match block {
                    ContentBlock::Text(text) => serde_json::json!({
                        "type": "text",
                        "text": text.text,
                    }),
                    ContentBlock::Image(image) => serde_json::json!({
                        "type": "image",
                        "source": image.source,
                    }),
                    ContentBlock::Document(document) => serde_json::json!({
                        "type": "document",
                        "source": document.source,
                    }),
                    _ => Value::Null,
                }
            }).collect(),
        };
        
        let mut message_obj = HashMap::new();
        message_obj.insert("role".to_string(), Value::String(role.to_string()));
        message_obj.insert("content".to_string(), Value::Array(content));
        
        openai_messages.push(serde_json::json!(message_obj));
    }
    
    serde_json::json!({ "messages": openai_messages })
}

/// Convert OpenAI messages to Anthropic format
pub fn convert_openai_to_anthropic(openai_response: &Value) -> Result<MessagesResponse, String> {
    let id = openai_response.get("id")
        .and_then(|v| v.as_str())
        .unwrap_or("msg_000000000000000000000000");
    
    let model = openai_response.get("model")
        .and_then(|v| v.as_str())
        .unwrap_or("gpt-3.5-turbo");
    
    let empty = Vec::new();
    let choices = openai_response.get("choices")
        .and_then(|v| v.as_array())
        .unwrap_or(&empty);
    
    let mut content = Vec::new();
    for choice in choices {
        if let Some(message) = choice.get("message") {
            if let Some(content_array) = message.get("content").and_then(|v| v.as_array()) {
                for item in content_array {
                    content.push(item.clone());
                }
            }
        }
    }
    
    let usage = openai_response.get("usage")
        .and_then(|v| v.as_object())
        .map(|obj| Usage {
            input_tokens: obj.get("prompt_tokens")
                .and_then(|v| v.as_i64())
                .unwrap_or(0) as i32,
            output_tokens: obj.get("completion_tokens")
                .and_then(|v| v.as_i64())
                .unwrap_or(0) as i32,
            cache_creation_input_tokens: obj.get("cache_creation_input_tokens")
                .and_then(|v| v.as_i64())
                .unwrap_or(0) as i32,
            cache_read_input_tokens: obj.get("cache_read_input_tokens")
                .and_then(|v| v.as_i64())
                .unwrap_or(0) as i32,
        })
        .unwrap_or_else(|| Usage {
            input_tokens: 0,
            output_tokens: 0,
            cache_creation_input_tokens: 0,
            cache_read_input_tokens: 0,
        });
    
    Ok(MessagesResponse::new(id.to_string(), model.to_string(), content, usage))
}