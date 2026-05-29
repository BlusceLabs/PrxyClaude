pub mod discord;
pub mod telegram;

use async_trait::async_trait;

use crate::messaging::models::IncomingMessage;

/// Abstract base for messaging platforms.
#[async_trait]
pub trait MessagingPlatform: Send + Sync {
    /// Platform name.
    fn name(&self) -> &str;

    /// Initialize and connect to the messaging platform.
    async fn start(&mut self) -> Result<(), String>;

    /// Disconnect and cleanup resources.
    async fn stop(&mut self) -> Result<(), String>;

    /// Send a message to a chat. Returns message ID.
    async fn send_message(
        &self,
        chat_id: &str,
        text: &str,
        reply_to: Option<&str>,
        parse_mode: Option<&str>,
        message_thread_id: Option<&str>,
    ) -> Result<String, String>;

    /// Edit an existing message.
    async fn edit_message(
        &self,
        chat_id: &str,
        message_id: &str,
        text: &str,
        parse_mode: Option<&str>,
    ) -> Result<(), String>;

    /// Delete a message from a chat.
    async fn delete_message(&self, chat_id: &str, message_id: &str) -> Result<(), String>;

    /// Enqueue a message to be sent.
    async fn queue_send_message(
        &self,
        chat_id: &str,
        text: &str,
        reply_to: Option<&str>,
        parse_mode: Option<&str>,
        fire_and_forget: bool,
        message_thread_id: Option<&str>,
    ) -> Result<Option<String>, String>;

    /// Enqueue a message edit.
    async fn queue_edit_message(
        &self,
        chat_id: &str,
        message_id: &str,
        text: &str,
        parse_mode: Option<&str>,
        fire_and_forget: bool,
    ) -> Result<(), String>;

    /// Enqueue a message deletion.
    async fn queue_delete_message(
        &self,
        chat_id: &str,
        message_id: &str,
        fire_and_forget: bool,
    ) -> Result<(), String>;

    /// Delete many messages.
    async fn queue_delete_messages(
        &self,
        chat_id: &str,
        message_ids: &[String],
        fire_and_forget: bool,
    ) -> Result<(), String>;

    /// Register a message handler callback.
    fn on_message(&mut self, handler: Box<dyn Fn(IncomingMessage) -> tokio::task::JoinHandle<()> + Send + Sync>);

    /// Check if the platform is connected.
    fn is_connected(&self) -> bool;
}

/// Protocol for session managers to avoid tight coupling.
#[async_trait]
pub trait SessionManagerInterface: Send + Sync {
    async fn get_or_create_session(
        &self,
        session_id: Option<&str>,
    ) -> Result<(String, String, bool), String>;

    async fn register_real_session_id(&self, temp_id: &str, real_session_id: &str) -> bool;

    async fn stop_all(&self) -> Result<(), String>;

    async fn remove_session(&self, session_id: &str) -> bool;

    fn get_stats(&self) -> serde_json::Value;
}

/// Create a platform adapter by name.
pub fn create_platform(name: &str, config: serde_json::Value) -> Result<Box<dyn MessagingPlatform>, String> {
    match name {
        "telegram" => {
            let bot_token = config
                .get("bot_token")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let allowed_user_id = config
                .get("allowed_user_id")
                .and_then(|v| v.as_str())
                .map(String::from);
            Ok(Box::new(telegram::TelegramPlatform::new(
                bot_token,
                allowed_user_id,
            )))
        }
        "discord" => {
            let bot_token = config
                .get("bot_token")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let allowed_channel_ids = config
                .get("allowed_channel_ids")
                .and_then(|v| v.as_str())
                .map(String::from);
            Ok(Box::new(discord::DiscordPlatform::new(
                bot_token,
                allowed_channel_ids,
            )))
        }
        _ => Err(format!("Unknown platform: {name}")),
    }
}
