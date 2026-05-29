use std::sync::Arc;

use crate::messaging::commands::{handle_clear_command, handle_stats_command, handle_stop_command};
use crate::messaging::models::IncomingMessage;
use crate::messaging::session::SessionStore;
use crate::messaging::trees::queue_manager::TreeQueueManager;
use crate::messaging::ui_updates::PlatformOps;

/// Return the slash command without bot mention suffix.
pub fn parse_command_base(text: Option<&str>) -> String {
    let text = text.unwrap_or("");
    let parts: Vec<&str> = text.trim().split_whitespace().collect();
    let cmd = parts.first().unwrap_or(&"");
    cmd.split('@').next().unwrap_or("").to_string()
}

/// Return the persistence kind for an incoming message.
pub fn message_kind_for_command(command_base: &str) -> &str {
    if command_base.starts_with('/') {
        "command"
    } else {
        "content"
    }
}

/// Dispatch a known command and return whether it was handled.
pub async fn dispatch_command(
    platform: Arc<dyn PlatformOps>,
    incoming: IncomingMessage,
    command_base: &str,
    tree_queue: Arc<TreeQueueManager>,
    session_store: Arc<SessionStore>,
    format_status: Arc<dyn Fn(&str, &str) -> String + Send + Sync>,
) -> bool {
    match command_base {
        "/stop" => {
            handle_stop_command(platform, incoming, tree_queue, session_store, format_status).await;
            true
        }
        "/stats" => {
            handle_stats_command(platform, incoming, tree_queue, session_store, format_status).await;
            true
        }
        "/clear" => {
            handle_clear_command(platform, incoming, tree_queue, session_store, format_status).await;
            true
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_command_base() {
        assert_eq!(parse_command_base(Some("/stop")), "/stop");
        assert_eq!(parse_command_base(Some("/stop@bot")), "/stop");
        assert_eq!(parse_command_base(Some("  /clear  ")), "/clear");
        assert_eq!(parse_command_base(None), "");
        // Non-command text returns the first word
        assert_eq!(parse_command_base(Some("hello")), "hello");
    }

    #[test]
    fn test_message_kind() {
        assert_eq!(message_kind_for_command("/stop"), "command");
        assert_eq!(message_kind_for_command("hello"), "content");
    }
}
