use std::sync::Arc;
use std::time::Instant;

use tracing::warn;

use crate::messaging::rendering::profiles::RenderCtx;
use crate::messaging::safe_diagnostics::format_exception_for_log;
use crate::messaging::transcript::TranscriptBuffer;

/// Trait for messaging platform operations needed by ThrottledTranscriptEditor.
#[async_trait::async_trait]
pub trait PlatformOps: Send + Sync {
    async fn queue_edit_message(
        &self,
        chat_id: &str,
        message_id: &str,
        text: &str,
        parse_mode: Option<&str>,
        fire_and_forget: bool,
    ) -> Result<(), String>;

    async fn queue_send_message(
        &self,
        chat_id: &str,
        text: &str,
        reply_to: Option<&str>,
        parse_mode: Option<&str>,
        fire_and_forget: bool,
        message_thread_id: Option<&str>,
    ) -> Result<Option<String>, String>;

    async fn queue_delete_message(
        &self,
        chat_id: &str,
        message_id: &str,
        fire_and_forget: bool,
    ) -> Result<(), String>;
}

/// Rate-limited status message edits from a growing transcript.
pub struct ThrottledTranscriptEditor<P: PlatformOps> {
    platform: Arc<P>,
    parse_mode: Option<String>,
    limit_chars: usize,
    transcript: TranscriptBuffer,
    render_ctx: RenderCtx,
    node_id: String,
    chat_id: String,
    status_msg_id: String,
    last_ui_update: Instant,
    last_displayed_text: Option<String>,
    last_status: Option<String>,
}

impl<P: PlatformOps> ThrottledTranscriptEditor<P> {
    pub fn new(
        platform: Arc<P>,
        parse_mode: Option<String>,
        limit_chars: usize,
        transcript: TranscriptBuffer,
        render_ctx: RenderCtx,
        node_id: String,
        chat_id: String,
        status_msg_id: String,
    ) -> Self {
        Self {
            platform,
            parse_mode,
            limit_chars,
            transcript,
            render_ctx,
            node_id,
            chat_id,
            status_msg_id,
            last_ui_update: Instant::now() - std::time::Duration::from_secs(10),
            last_displayed_text: None,
            last_status: None,
        }
    }

    pub fn last_status(&self) -> Option<&str> {
        self.last_status.as_deref()
    }

    pub async fn update(&mut self, status: Option<&str>, force: bool) {
        let now = Instant::now();
        if !force && now.duration_since(self.last_ui_update).as_secs_f64() < 1.0 {
            return;
        }

        self.last_ui_update = now;
        if let Some(s) = status {
            self.last_status = Some(s.to_string());
        }

        let display = self.transcript.render(
            &self.render_ctx,
            self.limit_chars,
            status,
        );

        if !display.is_empty() && self.last_displayed_text.as_deref() != Some(&display) {
            self.last_displayed_text = Some(display.clone());
            if let Err(e) = self
                .platform
                .queue_edit_message(
                    &self.chat_id,
                    &self.status_msg_id,
                    &display,
                    self.parse_mode.as_deref(),
                    false,
                )
                .await
            {
                warn!(
                    "Failed to update platform for node {}: {}",
                    self.node_id,
                    format_exception_for_log(&e, false)
                );
            }
        }
    }
}
