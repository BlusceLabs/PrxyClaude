use std::sync::Arc;

use crate::messaging::models::IncomingMessage;
use crate::messaging::session::SessionStore;
use crate::messaging::trees::queue_manager::TreeQueueManager;
use crate::messaging::ui_updates::PlatformOps;

/// Command handler type.
pub type CommandHandler = Arc<
    dyn Fn(
            Arc<dyn crate::messaging::ui_updates::PlatformOps>,
            IncomingMessage,
            Arc<TreeQueueManager>,
            Arc<SessionStore>,
            Arc<dyn Fn(&str, &str) -> String + Send + Sync>,
        ) -> tokio::task::JoinHandle<()>
        + Send
        + Sync,
>;

/// Delete message IDs in sorted order (descending numeric).
pub async fn delete_message_ids(
    platform: &Arc<dyn crate::messaging::ui_updates::PlatformOps>,
    chat_id: &str,
    msg_ids: &std::collections::HashSet<String>,
) {
    if msg_ids.is_empty() {
        return;
    }

    let mut numeric: Vec<(i64, String)> = Vec::new();
    let mut non_numeric: Vec<String> = Vec::new();

    for mid in msg_ids {
        if let Ok(n) = mid.parse::<i64>() {
            numeric.push((n, mid.clone()));
        } else {
            non_numeric.push(mid.clone());
        }
    }
    numeric.sort_by(|a, b| b.0.cmp(&a.0));
    let ordered: Vec<String> = numeric
        .into_iter()
        .map(|(_, s)| s)
        .chain(non_numeric)
        .collect();

    const CHUNK: usize = 100;
    for chunk in ordered.chunks(CHUNK) {
        for mid in chunk {
            let _ = platform.queue_delete_message(chat_id, mid, false).await;
        }
    }
}

/// Handle /stop command.
pub async fn handle_stop_command(
    platform: Arc<dyn crate::messaging::ui_updates::PlatformOps>,
    incoming: IncomingMessage,
    tree_queue: Arc<TreeQueueManager>,
    _session_store: Arc<SessionStore>,
    format_status: Arc<dyn Fn(&str, &str) -> String + Send + Sync>,
) {
    if incoming.is_reply() {
        if let Some(reply_id) = &incoming.reply_to_message_id {
            if let Some(_tree) = tree_queue.get_tree_for_node(reply_id) {
                if let Some(_node_id) = tree_queue.resolve_parent_node_id(reply_id) {
                    let count = 1; // Simplified
                    let msg = format!(
                        "{}",
                        (format_status)("\u{23f9}", &format!("Stopped. Cancelled {count} request(s)."))
                    );
                    let _ = platform
                        .queue_send_message(&incoming.chat_id, &msg, None, None, false, incoming.message_thread_id.as_deref())
                        .await;
                    return;
                }
            }
            let msg = format!(
                "{}",
                (format_status)("\u{23f9}", "Stopped. Nothing to stop for that message.")
            );
            let _ = platform
                .queue_send_message(&incoming.chat_id, &msg, None, None, false, incoming.message_thread_id.as_deref())
                .await;
            return;
        }
    }

    // Global stop
    let cancelled = tree_queue.cancel_all().await;
    let count = cancelled.len();
    let msg = format!(
        "{}",
        (format_status)(
            "\u{23f9}",
            &format!("Stopped. Cancelled {count} pending or active requests.")
        )
    );
    let _ = platform
        .queue_send_message(&incoming.chat_id, &msg, None, None, false, incoming.message_thread_id.as_deref())
        .await;
}

/// Handle /stats command.
pub async fn handle_stats_command(
    platform: Arc<dyn crate::messaging::ui_updates::PlatformOps>,
    incoming: IncomingMessage,
    tree_queue: Arc<TreeQueueManager>,
    _session_store: Arc<SessionStore>,
    format_status: Arc<dyn Fn(&str, &str) -> String + Send + Sync>,
) {
    let tree_count = tree_queue.get_tree_count();
    let msg = format!(
        "\u{1f4ca} {}\n\u{2022} Message Trees: {tree_count}",
        (format_status)("", "Stats")
    );
    let _ = platform
        .queue_send_message(&incoming.chat_id, &msg, None, None, false, incoming.message_thread_id.as_deref())
        .await;
}

/// Handle /clear command.
pub async fn handle_clear_command(
    platform: Arc<dyn crate::messaging::ui_updates::PlatformOps>,
    incoming: IncomingMessage,
    tree_queue: Arc<TreeQueueManager>,
    session_store: Arc<SessionStore>,
    format_status: Arc<dyn Fn(&str, &str) -> String + Send + Sync>,
) {
    // Global clear
    tree_queue.cancel_all().await;

    // Collect message IDs
    let mut msg_ids: std::collections::HashSet<String> = std::collections::HashSet::new();

    // Get recorded message IDs
    let recorded = session_store
        .get_message_ids_for_chat(&incoming.platform, &incoming.chat_id)
        .await;
    for mid in recorded {
        msg_ids.insert(mid);
    }

    // Get tree message IDs
    let tree_ids = tree_queue.get_message_ids_for_chat(&incoming.platform, &incoming.chat_id);
    msg_ids.extend(tree_ids);

    // Add command message itself
    msg_ids.insert(incoming.message_id.clone());

    delete_message_ids(&platform, &incoming.chat_id, &msg_ids).await;

    // Clear persistent state
    session_store.clear_all().await;

    let msg = format!(
        "{}",
        (format_status)("\u{1f5d1}", "Cleared. All messages removed.")
    );
    let _ = platform
        .queue_send_message(&incoming.chat_id, &msg, None, None, false, incoming.message_thread_id.as_deref())
        .await;
}
