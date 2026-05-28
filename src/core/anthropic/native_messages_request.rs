use crate::models::MessagesRequest;

/// Native messages request for Anthropic
pub struct NativeMessagesRequest {
    pub model: String,
    pub messages: Vec<crate::models::Message>,
    pub max_tokens: Option<i32>,
    pub stop_sequences: Option<Vec<String>>,
    pub system: Option<String>,
    pub temperature: Option<f64>,
    pub top_p: Option<f64>,
    pub top_k: Option<i32>,
    pub stream: Option<bool>,
}

impl NativeMessagesRequest {
    pub fn from_request(request: &MessagesRequest) -> Self {
        Self {
            model: request.model.clone(),
            messages: request.messages.clone(),
            max_tokens: request.max_tokens,
            stop_sequences: None, // Not in original request
            system: match &request.system {
                Some(sys) => match sys {
                    crate::models::SystemContentOrString::System(s) => Some(s.text.clone()),
                    crate::models::SystemContentOrString::String(s) => Some(s.clone()),
                },
                None => None,
            },
            temperature: request.temperature,
            top_p: request.top_p,
            top_k: None, // Not in original request
            stream: request.stream,
        }
    }
    
    // Clean up extra fields that might cause issues
    pub fn clean_extra_fields(&mut self) {
        // This would be used to clean up extra fields
        // For now, just a placeholder
    }
}