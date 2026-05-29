use std::sync::Arc;

use tracing::info;

use crate::messaging::constants::{get_status_for_event, TRANSCRIPT_EVENT_TYPES};
use crate::messaging::session::SessionStore;
use crate::messaging::transcript::TranscriptBuffer;
use crate::messaging::trees::data::{MessageState, SharedTree};

/// Handle session_info event; return updated (captured_session_id, temp_session_id).
pub async fn handle_session_info_event(
    event_data: &serde_json::Value,
    tree: Option<&SharedTree>,
    node_id: &str,
    captured_session_id: Option<String>,
    temp_session_id: Option<String>,
    session_store: &SessionStore,
) -> (Option<String>, Option<String>) {
    if event_data.get("type").and_then(|v| v.as_str()) != Some("session_info") {
        return (captured_session_id, temp_session_id);
    }

    let real_session_id = event_data
        .get("session_id")
        .and_then(|v| v.as_str())
        .map(String::from);

    match (real_session_id, temp_session_id) {
        (Some(real_id), Some(_temp_id)) => {
            if let Some(t) = tree {
                let mut tree_ref = t.write().await;
                tree_ref.update_state(
                    node_id,
                    MessageState::InProgress,
                    Some(real_id.clone()),
                    None,
                );
                let root_id = tree_ref.root_id.clone();
                let data = tree_ref.to_dict();
                drop(tree_ref);
                session_store.save_tree(&root_id, data).await;
            }
            (Some(real_id), None)
        }
        (Some(real_id), None) => (Some(real_id), None),
        (None, temp) => (captured_session_id, temp),
    }
}

/// Process a single parsed CLI event. Returns (last_status, had_transcript_events).
pub async fn process_parsed_cli_event(
    parsed: &serde_json::Value,
    transcript: &mut TranscriptBuffer,
    update_ui: &Arc<dyn Fn(Option<String>, bool) -> tokio::task::JoinHandle<()> + Send + Sync>,
    last_status: Option<String>,
    had_transcript_events: bool,
    tree: Option<&SharedTree>,
    node_id: &str,
    captured_session_id: Option<String>,
    session_store: &SessionStore,
    format_status: &Arc<dyn Fn(&str, &str) -> String + Send + Sync>,
    propagate_error_to_children: &Arc<
        dyn Fn(String, String, String) -> tokio::task::JoinHandle<()> + Send + Sync,
    >,
) -> (Option<String>, bool) {
    let ptype = parsed.get("type").and_then(|v| v.as_str()).unwrap_or("");
    let mut had_transcript_events = had_transcript_events;
    let mut last_status = last_status;

    if TRANSCRIPT_EVENT_TYPES.contains(&ptype) {
        transcript.apply(parsed);
        had_transcript_events = true;
    }

    let status = get_status_for_event(ptype, parsed, |e, l| format_status(e, l));
    if let Some(status) = status {
        update_ui(Some(status.clone()), false).await;
        last_status = Some(status);
    } else if ptype == "block_stop" {
        update_ui(last_status.clone(), true).await;
    } else if ptype == "complete" {
        if !had_transcript_events {
            transcript.apply(&serde_json::json!({"type": "text_chunk", "text": "Done."}));
        }
        info!("HANDLER: Task complete, updating UI");
        update_ui(Some(format_status("\u{2705}", "Complete")), true).await;
        if let (Some(t), Some(sid)) = (tree, captured_session_id) {
            let mut tree_ref = t.write().await;
            tree_ref.update_state(node_id, MessageState::Completed, Some(sid), None);
            let root_id = tree_ref.root_id.clone();
            let data = tree_ref.to_dict();
            drop(tree_ref);
            session_store.save_tree(&root_id, data).await;
        }
    } else if ptype == "error" {
        let error_msg = parsed
            .get("message")
            .and_then(|v| v.as_str())
            .unwrap_or("Unknown error")
            .to_string();
        info!("HANDLER: Error event received: message_chars={}", error_msg.len());
        update_ui(Some(format_status("\u{274c}", "Error")), true).await;
        if tree.is_some() {
            propagate_error_to_children(
                node_id.to_string(),
                error_msg,
                "Parent task failed".to_string(),
            )
            .await;
        }
    }

    (last_status, had_transcript_events)
}
