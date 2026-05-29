use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Platform-agnostic incoming message.
/// Adapters convert platform-specific events to this format.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IncomingMessage {
    pub text: String,
    pub chat_id: String,
    pub user_id: String,
    pub message_id: String,
    pub platform: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub reply_to_message_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message_thread_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status_message_id: Option<String>,

    #[serde(default = "chrono::Utc::now")]
    pub timestamp: DateTime<Utc>,
}

impl IncomingMessage {
    pub fn is_reply(&self) -> bool {
        self.reply_to_message_id.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_message(reply_to: Option<&str>) -> IncomingMessage {
        IncomingMessage {
            text: "hello".into(),
            chat_id: "123".into(),
            user_id: "u1".into(),
            message_id: "m1".into(),
            platform: "telegram".into(),
            reply_to_message_id: reply_to.map(String::from),
            message_thread_id: None,
            username: None,
            status_message_id: None,
            timestamp: Utc::now(),
        }
    }

    #[test]
    fn test_is_reply() {
        assert!(!make_message(None).is_reply());
        assert!(make_message(Some("m0")).is_reply());
    }

    #[test]
    fn test_serialization_roundtrip() {
        let msg = make_message(Some("m0"));
        let json = serde_json::to_string(&msg).unwrap();
        let decoded: IncomingMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.chat_id, "123");
        assert_eq!(decoded.reply_to_message_id, Some("m0".into()));
    }
}
