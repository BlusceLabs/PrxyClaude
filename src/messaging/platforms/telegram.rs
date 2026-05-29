use async_trait::async_trait;
use tracing::info;

use crate::messaging::models::IncomingMessage;

use super::MessagingPlatform;

/// Telegram messaging platform adapter.
pub struct TelegramPlatform {
    bot_token: String,
    allowed_user_id: Option<String>,
    connected: bool,
    handler: Option<Box<dyn Fn(IncomingMessage) -> tokio::task::JoinHandle<()> + Send + Sync>>,
    http_client: reqwest::Client,
}

impl TelegramPlatform {
    pub fn new(bot_token: String, allowed_user_id: Option<String>) -> Self {
        Self {
            bot_token,
            allowed_user_id,
            connected: false,
            handler: None,
            http_client: reqwest::Client::new(),
        }
    }

    async fn call_api(&self, method: &str, body: serde_json::Value) -> Result<serde_json::Value, String> {
        let url = format!("https://api.telegram.org/bot{}/{}", self.bot_token, method);
        let resp = self
            .http_client
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("HTTP error: {e}"))?;

        let status = resp.status();
        let body: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| format!("JSON parse error: {e}"))?;

        if !status.is_success() {
            return Err(format!("Telegram API error: {body}"));
        }

        Ok(body)
    }

    fn truncate(&self, text: &str) -> String {
        if text.len() <= 4096 {
            text.to_string()
        } else {
            format!("{}...", &text[..4093])
        }
    }
}

#[async_trait]
impl MessagingPlatform for TelegramPlatform {
    fn name(&self) -> &str {
        "telegram"
    }

    async fn start(&mut self) -> Result<(), String> {
        if self.bot_token.is_empty() {
            return Err("TELEGRAM_BOT_TOKEN is required".to_string());
        }

        // Test connection
        let result = self.call_api("getMe", serde_json::json!({})).await;
        match result {
            Ok(_) => {
                self.connected = true;
                info!("Telegram platform started (Bot API)");
                Ok(())
            }
            Err(e) => Err(format!("Failed to connect: {e}")),
        }
    }

    async fn stop(&mut self) -> Result<(), String> {
        self.connected = false;
        info!("Telegram platform stopped");
        Ok(())
    }

    async fn send_message(
        &self,
        chat_id: &str,
        text: &str,
        reply_to: Option<&str>,
        parse_mode: Option<&str>,
        message_thread_id: Option<&str>,
    ) -> Result<String, String> {
        let text = self.truncate(text);
        let mut body = serde_json::json!({
            "chat_id": chat_id,
            "text": text,
        });

        if let Some(rt) = reply_to {
            if let Ok(id) = rt.parse::<i64>() {
                body["reply_to_message_id"] = serde_json::json!(id);
            }
        }
        if let Some(pm) = parse_mode {
            body["parse_mode"] = serde_json::json!(pm);
        }
        if let Some(tid) = message_thread_id {
            if let Ok(id) = tid.parse::<i64>() {
                body["message_thread_id"] = serde_json::json!(id);
            }
        }

        let result = self.call_api("sendMessage", body).await?;
        let msg_id = result["result"]["message_id"]
            .as_i64()
            .unwrap_or(0)
            .to_string();
        Ok(msg_id)
    }

    async fn edit_message(
        &self,
        chat_id: &str,
        message_id: &str,
        text: &str,
        parse_mode: Option<&str>,
    ) -> Result<(), String> {
        let text = self.truncate(text);
        let mut body = serde_json::json!({
            "chat_id": chat_id,
            "message_id": message_id.parse::<i64>().unwrap_or(0),
            "text": text,
        });

        if let Some(pm) = parse_mode {
            body["parse_mode"] = serde_json::json!(pm);
        }

        self.call_api("editMessageText", body).await?;
        Ok(())
    }

    async fn delete_message(&self, chat_id: &str, message_id: &str) -> Result<(), String> {
        let body = serde_json::json!({
            "chat_id": chat_id,
            "message_id": message_id.parse::<i64>().unwrap_or(0),
        });
        self.call_api("deleteMessage", body).await?;
        Ok(())
    }

    async fn queue_send_message(
        &self,
        chat_id: &str,
        text: &str,
        reply_to: Option<&str>,
        parse_mode: Option<&str>,
        fire_and_forget: bool,
        message_thread_id: Option<&str>,
    ) -> Result<Option<String>, String> {
        if fire_and_forget {
            let this = self.clone_handle();
            let chat_id = chat_id.to_string();
            let text = text.to_string();
            let reply_to = reply_to.map(String::from);
            let parse_mode = parse_mode.map(String::from);
            let message_thread_id = message_thread_id.map(String::from);
            tokio::spawn(async move {
                let _ = this
                    .send_message(
                        &chat_id,
                        &text,
                        reply_to.as_deref(),
                        parse_mode.as_deref(),
                        message_thread_id.as_deref(),
                    )
                    .await;
            });
            Ok(None)
        } else {
            let msg_id = self
                .send_message(chat_id, text, reply_to, parse_mode, message_thread_id)
                .await?;
            Ok(Some(msg_id))
        }
    }

    async fn queue_edit_message(
        &self,
        chat_id: &str,
        message_id: &str,
        text: &str,
        parse_mode: Option<&str>,
        fire_and_forget: bool,
    ) -> Result<(), String> {
        if fire_and_forget {
            let this = self.clone_handle();
            let chat_id = chat_id.to_string();
            let message_id = message_id.to_string();
            let text = text.to_string();
            let parse_mode = parse_mode.map(String::from);
            tokio::spawn(async move {
                let _ = this
                    .edit_message(&chat_id, &message_id, &text, parse_mode.as_deref())
                    .await;
            });
            Ok(())
        } else {
            self.edit_message(chat_id, message_id, text, parse_mode)
                .await
        }
    }

    async fn queue_delete_message(
        &self,
        chat_id: &str,
        message_id: &str,
        fire_and_forget: bool,
    ) -> Result<(), String> {
        if fire_and_forget {
            let this = self.clone_handle();
            let chat_id = chat_id.to_string();
            let message_id = message_id.to_string();
            tokio::spawn(async move {
                let _ = this.delete_message(&chat_id, &message_id).await;
            });
            Ok(())
        } else {
            self.delete_message(chat_id, message_id).await
        }
    }

    async fn queue_delete_messages(
        &self,
        chat_id: &str,
        message_ids: &[String],
        fire_and_forget: bool,
    ) -> Result<(), String> {
        for mid in message_ids {
            self.queue_delete_message(chat_id, mid, fire_and_forget)
                .await?;
        }
        Ok(())
    }

    fn on_message(&mut self, handler: Box<dyn Fn(IncomingMessage) -> tokio::task::JoinHandle<()> + Send + Sync>) {
        self.handler = Some(handler);
    }

    fn is_connected(&self) -> bool {
        self.connected
    }
}

impl TelegramPlatform {
    /// Clone a handle for spawning tasks (simplified - in production use Arc).
    fn clone_handle(&self) -> TelegramPlatform {
        TelegramPlatform {
            bot_token: self.bot_token.clone(),
            allowed_user_id: self.allowed_user_id.clone(),
            connected: self.connected,
            handler: None,
            http_client: self.http_client.clone(),
        }
    }
}
