use async_trait::async_trait;
use tracing::info;

use crate::messaging::models::IncomingMessage;

use super::MessagingPlatform;

const DISCORD_MESSAGE_LIMIT: usize = 2000;

/// Discord messaging platform adapter.
pub struct DiscordPlatform {
    bot_token: String,
    allowed_channel_ids: Option<String>,
    connected: bool,
    handler: Option<Box<dyn Fn(IncomingMessage) -> tokio::task::JoinHandle<()> + Send + Sync>>,
    http_client: reqwest::Client,
}

impl DiscordPlatform {
    pub fn new(bot_token: String, allowed_channel_ids: Option<String>) -> Self {
        Self {
            bot_token,
            allowed_channel_ids,
            connected: false,
            handler: None,
            http_client: reqwest::Client::new(),
        }
    }

    async fn call_api(
        &self,
        method: &str,
        path: &str,
        body: Option<serde_json::Value>,
    ) -> Result<serde_json::Value, String> {
        let url = format!("https://discord.com/api/v10{path}");
        let mut req = self
            .http_client
            .request(method.parse().unwrap(), &url)
            .header("Authorization", format!("Bot {}", self.bot_token))
            .header("Content-Type", "application/json");

        if let Some(body) = body {
            req = req.json(&body);
        }

        let resp = req.send().await.map_err(|e| format!("HTTP error: {e}"))?;

        if resp.status().is_success() {
            if let Ok(text) = resp.text().await {
                if text.is_empty() || text == "null" {
                    return Ok(serde_json::json!({}));
                }
                serde_json::from_str(&text).map_err(|e| format!("JSON parse error: {e}"))
            } else {
                Ok(serde_json::json!({}))
            }
        } else {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            Err(format!("Discord API error ({status}): {text}"))
        }
    }

    fn truncate(&self, text: &str) -> String {
        if text.len() <= DISCORD_MESSAGE_LIMIT {
            text.to_string()
        } else {
            format!("{}...", &text[..DISCORD_MESSAGE_LIMIT - 3])
        }
    }

    fn parse_allowed_channels(raw: &str) -> std::collections::HashSet<String> {
        raw.split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect()
    }
}

#[async_trait]
impl MessagingPlatform for DiscordPlatform {
    fn name(&self) -> &str {
        "discord"
    }

    async fn start(&mut self) -> Result<(), String> {
        if self.bot_token.is_empty() {
            return Err("DISCORD_BOT_TOKEN is required".to_string());
        }

        // Test connection by getting current user
        match self.call_api("GET", "/users/@me", None).await {
            Ok(_) => {
                self.connected = true;
                info!("Discord platform started");
                Ok(())
            }
            Err(e) => Err(format!("Failed to connect: {e}")),
        }
    }

    async fn stop(&mut self) -> Result<(), String> {
        self.connected = false;
        info!("Discord platform stopped");
        Ok(())
    }

    async fn send_message(
        &self,
        chat_id: &str,
        text: &str,
        reply_to: Option<&str>,
        _parse_mode: Option<&str>,
        _message_thread_id: Option<&str>,
    ) -> Result<String, String> {
        let text = self.truncate(text);
        let mut body = serde_json::json!({
            "content": text,
        });

        if let Some(rt) = reply_to {
            if let Ok(id) = rt.parse::<u64>() {
                body["message_reference"] = serde_json::json!({
                    "message_id": id.to_string(),
                    "channel_id": chat_id,
                });
            }
        }

        let result = self
            .call_api("POST", &format!("/channels/{chat_id}/messages"), Some(body))
            .await?;
        let msg_id = result["id"].as_str().unwrap_or("0").to_string();
        Ok(msg_id)
    }

    async fn edit_message(
        &self,
        chat_id: &str,
        message_id: &str,
        text: &str,
        _parse_mode: Option<&str>,
    ) -> Result<(), String> {
        let text = self.truncate(text);
        let body = serde_json::json!({
            "content": text,
        });

        self.call_api(
            "PATCH",
            &format!("/channels/{chat_id}/messages/{message_id}"),
            Some(body),
        )
        .await?;
        Ok(())
    }

    async fn delete_message(&self, chat_id: &str, message_id: &str) -> Result<(), String> {
        self.call_api(
            "DELETE",
            &format!("/channels/{chat_id}/messages/{message_id}"),
            None,
        )
        .await?;
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

impl DiscordPlatform {
    fn clone_handle(&self) -> DiscordPlatform {
        DiscordPlatform {
            bot_token: self.bot_token.clone(),
            allowed_channel_ids: self.allowed_channel_ids.clone(),
            connected: self.connected,
            handler: None,
            http_client: self.http_client.clone(),
        }
    }
}
