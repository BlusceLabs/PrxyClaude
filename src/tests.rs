#[cfg(test)]
mod tests {
    use crate::config::Config;
    use crate::models::MessagesRequest;
    use crate::models::Role;
    use crate::models::Message;
    use crate::models::ContentOrBlocks;
    
    #[test]
    fn test_config_creation() {
        let config = Config::default();
        assert_eq!(config.server.addr, "127.0.0.1:8080");
        assert_eq!(config.providers.default_provider, "open_router");
    }
    
    #[test]
    fn test_message_creation() {
        let message = Message::new(
            Role::User,
            ContentOrBlocks::String("Hello, world!".to_string())
        );
        
        assert_eq!(message.role, Role::User);
        if let ContentOrBlocks::String(text) = &message.content {
            assert_eq!(text, "Hello, world!");
        } else {
            panic!("Expected string content");
        }
    }
    
    #[test]
    fn test_messages_request_creation() {
        let messages = vec![
            Message::new(
                Role::User,
                ContentOrBlocks::String("Hello!".to_string())
            )
        ];
        
        let request = MessagesRequest::new("gpt-3.5-turbo".to_string(), messages);
        assert_eq!(request.model, "gpt-3.5-turbo");
        assert_eq!(request.messages.len(), 1);
    }
}