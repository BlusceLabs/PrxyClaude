use std::sync::Arc;

use tracing::{error, info, warn};

use crate::messaging::command_dispatcher::{dispatch_command, message_kind_for_command, parse_command_base};
use crate::messaging::constants::is_status_message;
use crate::messaging::models::IncomingMessage;
use crate::messaging::rendering::profiles::{build_rendering_profile, RenderingProfile};
use crate::messaging::session::SessionStore;
use crate::messaging::trees::data::SharedTree;
use crate::messaging::trees::queue_manager::TreeQueueManager;
use crate::messaging::ui_updates::PlatformOps;


/// Platform-agnostic handler for Claude interactions.
pub struct ClaudeMessageHandler {
    platform: Arc<dyn PlatformOps>,
    session_store: Arc<SessionStore>,
    tree_queue: Arc<TreeQueueManager>,
    rendering_profile: Arc<RenderingProfile>,
}

impl ClaudeMessageHandler {
    pub fn new(
        platform: Arc<dyn PlatformOps>,
        session_store: Arc<SessionStore>,
        tree_queue: Arc<TreeQueueManager>,
        platform_name: &str,
    ) -> Self {
        let rendering_profile = build_rendering_profile(platform_name);
        info!("ClaudeMessageHandler initialized");
        Self {
            platform,
            session_store,
            tree_queue,
            rendering_profile,
        }
    }

    pub fn format_status(&self, emoji: &str, label: &str, suffix: Option<&str>) -> String {
        (self.rendering_profile.format_status)(emoji, label, suffix)
    }

    pub fn parse_mode(&self) -> Option<&str> {
        self.rendering_profile.parse_mode.as_deref()
    }

    pub fn limit_chars(&self) -> usize {
        self.rendering_profile.limit_chars
    }

    /// Main entry point for handling an incoming message.
    pub async fn handle_message(&self, incoming: IncomingMessage) {
        let raw = &incoming.text;
        info!(
            "HANDLER_ENTRY: chat_id={} message_id={} reply_to={} text_len={}",
            incoming.chat_id,
            incoming.message_id,
            incoming.reply_to_message_id.as_deref().unwrap_or(""),
            raw.len(),
        );

        self.handle_message_impl(incoming).await;
    }

    async fn handle_message_impl(&self, incoming: IncomingMessage) {
        let cmd_base = parse_command_base(Some(&incoming.text));

        // Record incoming message ID
        if !incoming.message_id.is_empty() {
            self.session_store
                .record_message_id(
                    &incoming.platform,
                    &incoming.chat_id,
                    &incoming.message_id,
                    "in",
                    message_kind_for_command(&cmd_base),
                )
                .await;
        }

        // Dispatch commands
        if dispatch_command(
            self.platform.clone(),
            incoming.clone(),
            &cmd_base,
            self.tree_queue.clone(),
            self.session_store.clone(),
            Arc::new({
                let rp = self.rendering_profile.clone();
                move |e, l| (rp.format_status)(e, l, None)
            }),
        )
        .await
        {
            return;
        }

        // Filter out status messages
        if is_status_message(&incoming.text) {
            return;
        }

        // Check if this is a reply to an existing node
        let mut parent_node_id = None;
        let mut tree: Option<SharedTree> = None;

        if incoming.is_reply() {
            if let Some(reply_id) = &incoming.reply_to_message_id {
                if let Some(t) = self.tree_queue.get_tree_for_node(reply_id) {
                    if let Some(node_id) = self.tree_queue.resolve_parent_node_id(reply_id) {
                        info!("Found tree for reply, parent node: {node_id}");
                        parent_node_id = Some(node_id);
                        tree = Some(t);
                    } else {
                        warn!(
                            "Reply to {reply_id} found tree but no valid parent node"
                        );
                        tree = None;
                    }
                }
            }
        }

        let node_id = incoming.message_id.clone();

        // Get initial status
        let status_text = self.get_initial_status(tree.as_ref(), parent_node_id.as_deref());

        // Send or edit status message
        let status_msg_id = if let Some(ref sid) = incoming.status_message_id {
            let _ = self
                .platform
                .queue_edit_message(
                    &incoming.chat_id,
                    sid,
                    &status_text,
                    self.parse_mode(),
                    false,
                )
                .await;
            sid.clone()
        } else {
            match self
                .platform
                .queue_send_message(
                    &incoming.chat_id,
                    &status_text,
                    Some(&incoming.message_id),
                    self.parse_mode(),
                    false,
                    incoming.message_thread_id.as_deref(),
                )
                .await
            {
                Ok(Some(id)) => id,
                _ => return,
            }
        };

        self.record_outgoing_message(
            &incoming.platform,
            &incoming.chat_id,
            &status_msg_id,
            "status",
        )
        .await;

        // Create or extend tree
        if let (Some(parent_id), Some(_t), true) = (
            parent_node_id.as_deref(),
            tree.as_ref(),
            !status_msg_id.is_empty(),
        ) {
            // Reply to existing node
            match self
                .tree_queue
                .add_to_tree(
                    parent_id,
                    node_id.clone(),
                    incoming.clone(),
                    status_msg_id.clone(),
                )
                .await
            {
                Ok((tree_arc, _node)) => {
                    tree = Some(tree_arc.clone());
                    self.tree_queue.register_node(&status_msg_id, &tree_arc.read().await.root_id);
                    self.session_store
                        .register_node(&status_msg_id, &tree_arc.read().await.root_id)
                        .await;
                    self.session_store
                        .register_node(&node_id, &tree_arc.read().await.root_id)
                        .await;
                }
                Err(e) => {
                    error!("Failed to add to tree: {e}");
                }
            }
        } else if !status_msg_id.is_empty() {
            // New conversation
            let tree_arc = self
                .tree_queue
                .create_tree(
                    node_id.clone(),
                    incoming.clone(),
                    status_msg_id.clone(),
                )
                .await;
            self.tree_queue.register_node(&status_msg_id, &tree_arc.read().await.root_id);
            self.session_store
                .register_node(&node_id, &tree_arc.read().await.root_id)
                .await;
            self.session_store
                .register_node(&status_msg_id, &tree_arc.read().await.root_id)
                .await;
            tree = Some(tree_arc);
        }

        // Persist tree
        if let Some(t) = &tree {
            let data = t.read().await.to_dict();
            let root_id = t.read().await.root_id.clone();
            self.session_store.save_tree(&root_id, data).await;
        }

        // Enqueue for processing
        let queue_size = self.tree_queue.get_queue_size(&node_id);
        if queue_size > 0 && !status_msg_id.is_empty() {
            let status = self.format_status(
                "\u{1f4cb}",
                "Queued",
                Some(&format!("(position {queue_size}) - waiting...")),
            );
            let _ = self
                .platform
                .queue_edit_message(
                    &incoming.chat_id,
                    &status_msg_id,
                    &status,
                    self.parse_mode(),
                    true,
                )
                .await;
        }
    }

    fn get_initial_status(&self, tree: Option<&SharedTree>, parent_node_id: Option<&str>) -> String {
        if let (Some(_t), Some(parent_id)) = (tree, parent_node_id) {
            if self.tree_queue.is_node_tree_busy(parent_id) {
                let queue_size = self.tree_queue.get_queue_size(parent_id) + 1;
                return self.format_status(
                    "\u{1f4cb}",
                    "Queued",
                    Some(&format!("(position {queue_size}) - waiting...")),
                );
            }
            return self.format_status("\u{1f504}", "Continuing conversation...", None);
        }

        self.format_status(
            "\u{23f3}",
            "Launching new Claude CLI instance...",
            None,
        )
    }

    async fn record_outgoing_message(
        &self,
        platform: &str,
        chat_id: &str,
        msg_id: &str,
        kind: &str,
    ) {
        if msg_id.is_empty() {
            return;
        }
        self.session_store
            .record_message_id(platform, chat_id, msg_id, "out", kind)
            .await;
    }

    pub async fn stop_all_tasks(&self) -> usize {
        info!("Cancelling tree queue tasks...");
        let cancelled_nodes = self.tree_queue.cancel_all().await;
        let count = cancelled_nodes.len();
        info!("Cancelled {count} nodes");

        // Update UI for cancelled nodes
        for node in &cancelled_nodes {
            let status = self.format_status("\u{23f9}", "Stopped.", None);
            let _ = self
                .platform
                .queue_edit_message(
                    &node.incoming.chat_id,
                    &node.status_message_id,
                    &status,
                    self.parse_mode(),
                    true,
                )
                .await;
        }

        count
    }
}

/// Platform trait implementation for Arc<dyn PlatformOps>.
/// This is used for passing platform to command handlers.
#[async_trait::async_trait]
pub trait PlatformOpsExt: Send + Sync {
    async fn send_message(
        &self,
        chat_id: &str,
        text: &str,
        reply_to: Option<&str>,
        parse_mode: Option<&str>,
        message_thread_id: Option<&str>,
    ) -> Result<String, String>;

    async fn edit_message(
        &self,
        chat_id: &str,
        message_id: &str,
        text: &str,
        parse_mode: Option<&str>,
    ) -> Result<(), String>;

    async fn delete_message(&self, chat_id: &str, message_id: &str) -> Result<(), String>;

    async fn queue_send_message(
        &self,
        chat_id: &str,
        text: &str,
        reply_to: Option<&str>,
        parse_mode: Option<&str>,
        fire_and_forget: bool,
        message_thread_id: Option<&str>,
    ) -> Result<Option<String>, String>;

    async fn queue_edit_message(
        &self,
        chat_id: &str,
        message_id: &str,
        text: &str,
        parse_mode: Option<&str>,
        fire_and_forget: bool,
    ) -> Result<(), String>;

    async fn queue_delete_message(
        &self,
        chat_id: &str,
        message_id: &str,
        fire_and_forget: bool,
    ) -> Result<(), String>;

    fn is_connected(&self) -> bool;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_status() {
        // This is a basic test - full test requires a mock platform
        let rp = build_rendering_profile("telegram");
        let status = (rp.format_status)("\u{23f3}", "Processing", None);
        assert!(status.contains("Processing"));
    }
}
