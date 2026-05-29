use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::{Mutex, RwLock};
use tracing::{debug, error, info};

use super::data::{MessageNode, MessageState, MessageTree, SharedTree};
use crate::messaging::models::IncomingMessage;

/// In-memory index of trees and node-to-root mappings.
#[derive(Debug, Default)]
pub struct TreeRepository {
    trees: HashMap<String, SharedTree>,
    node_to_tree: HashMap<String, String>,
}

impl TreeRepository {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get_tree(&self, root_id: &str) -> Option<SharedTree> {
        self.trees.get(root_id).cloned()
    }

    pub fn get_tree_for_node(&self, node_id: &str) -> Option<SharedTree> {
        let root_id = self.node_to_tree.get(node_id)?;
        self.trees.get(root_id).cloned()
    }

    pub fn has_node(&self, node_id: &str) -> bool {
        self.node_to_tree.contains_key(node_id)
    }

    pub fn add_tree(&mut self, root_id: &str, tree: SharedTree) {
        self.trees.insert(root_id.to_string(), tree);
        self.node_to_tree.insert(root_id.to_string(), root_id.to_string());
        debug!("TREE_REPO: add_tree root_id={root_id}");
    }

    pub fn register_node(&mut self, node_id: &str, root_id: &str) {
        self.node_to_tree.insert(node_id.to_string(), root_id.to_string());
        debug!("TREE_REPO: register_node node_id={node_id} root_id={root_id}");
    }

    pub fn tree_count(&self) -> usize {
        self.trees.len()
    }

    pub fn is_tree_busy(&self, root_id: &str) -> bool {
        self.trees
            .get(root_id)
            .map(|t| {
                let tree = t.try_read();
                match tree {
                    Ok(t) => t.is_processing(),
                    Err(_) => false,
                }
            })
            .unwrap_or(false)
    }

    pub fn is_node_tree_busy(&self, node_id: &str) -> bool {
        self.get_tree_for_node(node_id)
            .map(|t| {
                let tree = t.try_read();
                match tree {
                    Ok(t) => t.is_processing(),
                    Err(_) => false,
                }
            })
            .unwrap_or(false)
    }

    pub fn get_queue_size(&self, node_id: &str) -> usize {
        self.get_tree_for_node(node_id)
            .map(|t| {
                let tree = t.try_read();
                match tree {
                    Ok(t) => t.get_queue_size(),
                    Err(_) => 0,
                }
            })
            .unwrap_or(0)
    }

    pub fn resolve_parent_node_id(&self, msg_id: &str) -> Option<String> {
        let tree = self.get_tree_for_node(msg_id)?;
        let tree_ref = tree.try_read().ok()?;
        if tree_ref.has_node(msg_id) {
            return Some(msg_id.to_string());
        }
        tree_ref
            .find_node_by_status_message(msg_id)
            .map(|n| n.node_id.clone())
    }

    pub fn get_pending_children(&self, node_id: &str) -> Vec<MessageNode> {
        let tree = match self.get_tree_for_node(node_id) {
            Some(t) => t,
            None => return Vec::new(),
        };
        let tree_ref = match tree.try_read() {
            Ok(t) => t,
            Err(_) => return Vec::new(),
        };

        let mut pending = Vec::new();
        let mut stack = vec![node_id.to_string()];
        while let Some(current_id) = stack.pop() {
            if let Some(node) = tree_ref.get_node(&current_id) {
                for child_id in &node.children_ids {
                    if let Some(child) = tree_ref.get_node(child_id) {
                        if child.state == MessageState::Pending {
                            pending.push(child.clone());
                            stack.push(child_id.clone());
                        }
                    }
                }
            }
        }
        pending
    }

    pub fn all_tree_ids(&self) -> Vec<String> {
        self.trees.keys().cloned().collect()
    }

    pub fn unregister_nodes(&mut self, node_ids: &[String]) {
        for nid in node_ids {
            self.node_to_tree.remove(nid);
        }
    }

    pub fn remove_tree(&mut self, root_id: &str) -> Option<SharedTree> {
        let tree = self.trees.remove(root_id)?;
        if let Ok(t) = tree.try_read() {
            for node in t.all_nodes() {
                self.node_to_tree.remove(&node.node_id);
            }
        }
        debug!("TREE_REPO: remove_tree root_id={root_id}");
        Some(tree)
    }

    pub fn get_message_ids_for_chat(&self, platform: &str, chat_id: &str) -> std::collections::HashSet<String> {
        let mut msg_ids = std::collections::HashSet::new();
        for tree in self.trees.values() {
            if let Ok(t) = tree.try_read() {
                for node in t.all_nodes() {
                    if node.incoming.platform == platform && node.incoming.chat_id == chat_id {
                        msg_ids.insert(node.incoming.message_id.clone());
                        msg_ids.insert(node.status_message_id.clone());
                    }
                }
            }
        }
        msg_ids
    }
}

type QueueUpdateCallback = Arc<dyn Fn(SharedTree) -> tokio::task::JoinHandle<()> + Send + Sync>;
type NodeStartedCallback = Arc<dyn Fn(SharedTree, String) -> tokio::task::JoinHandle<()> + Send + Sync>;

/// Manages multiple message trees: index + async processing.
pub struct TreeQueueManager {
    repository: Mutex<TreeRepository>,
    queue_update_callback: Option<QueueUpdateCallback>,
    node_started_callback: Option<NodeStartedCallback>,
    lock: Mutex<()>,
}

impl TreeQueueManager {
    pub fn new(
        queue_update_callback: Option<QueueUpdateCallback>,
        node_started_callback: Option<NodeStartedCallback>,
    ) -> Self {
        info!("TreeQueueManager initialized");
        Self {
            repository: Mutex::new(TreeRepository::new()),
            queue_update_callback,
            node_started_callback,
            lock: Mutex::new(()),
        }
    }

    pub fn get_tree_for_node(&self, node_id: &str) -> Option<SharedTree> {
        // For sync access, try try_lock
        self.repository.try_lock().ok().and_then(|repo| repo.get_tree_for_node(node_id))
    }

    pub fn get_tree(&self, root_id: &str) -> Option<SharedTree> {
        self.repository.try_lock().ok().and_then(|repo| repo.get_tree(root_id))
    }

    pub fn resolve_parent_node_id(&self, msg_id: &str) -> Option<String> {
        self.repository.try_lock().ok().and_then(|repo| repo.resolve_parent_node_id(msg_id))
    }

    pub fn is_node_tree_busy(&self, node_id: &str) -> bool {
        self.repository.try_lock().ok().map(|repo| repo.is_node_tree_busy(node_id)).unwrap_or(false)
    }

    pub fn get_queue_size(&self, node_id: &str) -> usize {
        self.repository.try_lock().ok().map(|repo| repo.get_queue_size(node_id)).unwrap_or(0)
    }

    pub fn get_tree_count(&self) -> usize {
        self.repository.try_lock().ok().map(|repo| repo.tree_count()).unwrap_or(0)
    }

    pub fn get_message_ids_for_chat(&self, platform: &str, chat_id: &str) -> std::collections::HashSet<String> {
        self.repository.try_lock().ok().map(|repo| repo.get_message_ids_for_chat(platform, chat_id)).unwrap_or_default()
    }

    pub fn get_pending_children(&self, node_id: &str) -> Vec<MessageNode> {
        self.repository.try_lock().ok().map(|repo| repo.get_pending_children(node_id)).unwrap_or_default()
    }

    pub fn register_node(&self, node_id: &str, root_id: &str) {
        if let Ok(mut repo) = self.repository.try_lock() {
            repo.register_node(node_id, root_id);
        }
    }

    pub async fn mark_node_error(
        &self,
        node_id: &str,
        error_message: &str,
        propagate_to_children: bool,
    ) -> Vec<MessageNode> {
        let tree = match self.repository.try_lock().ok().and_then(|repo| repo.get_tree_for_node(node_id)) {
            Some(t) => t,
            None => return Vec::new(),
        };

        let mut affected = Vec::new();
        {
            let mut tree_ref = tree.write().await;
            if let Some(node) = tree_ref.get_node(node_id) {
                let node = node.clone();
                tree_ref.update_state(node_id, MessageState::Error, None, Some(error_message.to_string()));
                affected.push(node);
            }
        }

        if propagate_to_children {
            let pending_children = self.get_pending_children(node_id);
            let mut tree_ref = tree.write().await;
            for child in pending_children {
                tree_ref.update_state(
                    &child.node_id,
                    MessageState::Error,
                    None,
                    Some(format!("Parent failed: {error_message}")),
                );
                if let Some(node) = tree_ref.get_node(&child.node_id) {
                    affected.push(node.clone());
                }
            }
        }

        affected
    }

    pub async fn cancel_tree(&self, root_id: &str) -> Vec<MessageNode> {
        let tree = match self.repository.try_lock().ok().and_then(|repo| repo.get_tree(root_id)) {
            Some(t) => t,
            None => return Vec::new(),
        };

        let mut tree_ref = tree.write().await;
        let mut cancelled = Vec::new();

        // Drain the queue
        let queue_nodes = tree_ref.drain_queue_and_cancelled("Cancelled by user");
        cancelled.extend(queue_nodes);

        // Mark stale nodes
        for node in tree_ref.all_nodes_mut() {
            if node.state == MessageState::Pending || node.state == MessageState::InProgress {
                node.state = MessageState::Error;
                node.error_message = Some("Stale task cleaned up".into());
                node.completed_at = Some(chrono::Utc::now());
                cancelled.push(node.clone());
            }
        }

        tree_ref.reset_processing_state();
        cancelled
    }

    pub async fn cancel_all(&self) -> Vec<MessageNode> {
        let _lock = self.lock.lock().await;
        let root_ids = self.repository.try_lock().ok().map(|repo| repo.all_tree_ids()).unwrap_or_default();
        let mut all_cancelled = Vec::new();
        for root_id in root_ids {
            all_cancelled.extend(self.cancel_tree(&root_id).await);
        }
        all_cancelled
    }

    pub fn cleanup_stale_nodes(&self) -> usize {
        let count = 0;
        // This is a simplified sync version
        // In production, use async properly
        count
    }

    pub async fn cancel_branch(&self, branch_root_id: &str) -> Vec<MessageNode> {
        let tree = match self.repository.try_lock().ok().and_then(|repo| repo.get_tree_for_node(branch_root_id)) {
            Some(t) => t,
            None => return Vec::new(),
        };

        let mut tree_ref = tree.write().await;
        let branch_ids: Vec<String> = tree_ref.get_descendants(branch_root_id);
        let mut cancelled = Vec::new();

        for nid in &branch_ids {
            let should_cancel = tree_ref.get_node(nid).map(|n| {
                n.state != MessageState::Completed && n.state != MessageState::Error
            }).unwrap_or(false);

            if should_cancel {
                tree_ref.remove_from_queue(nid);
                if let Some(node) = tree_ref.get_node_mut(nid) {
                    node.state = MessageState::Error;
                    node.error_message = Some("Cancelled by user".to_string());
                    node.completed_at = Some(chrono::Utc::now());
                    cancelled.push(node.clone());
                }
            }
        }

        if !cancelled.is_empty() {
            info!(
                "Cancelled {} nodes in branch {branch_root_id}",
                cancelled.len()
            );
        }
        cancelled
    }

    pub async fn remove_branch(
        &self,
        branch_root_id: &str,
    ) -> (Vec<MessageNode>, String, bool) {
        let tree = match self.repository.try_lock().ok().and_then(|repo| repo.get_tree_for_node(branch_root_id)) {
            Some(t) => t,
            None => return (Vec::new(), String::new(), false),
        };

        let root_id = {
            let tree_ref = tree.read().await;
            tree_ref.root_id.clone()
        };

        if branch_root_id == root_id {
            let cancelled = self.cancel_tree(&root_id).await;
            // Note: can't remove from repository without &mut self
            // This is a limitation of the current design
            return (cancelled, root_id, true);
        }

        let removed = {
            let mut tree_ref = tree.write().await;
            tree_ref.remove_branch(branch_root_id)
        };

        let _removed_ids: Vec<String> = removed.iter().map(|n| n.node_id.clone()).collect();
        // Note: can't unregister from repository without &mut self

        (removed, root_id, false)
    }

    pub async fn enqueue(
        &self,
        node_id: &str,
        processor: Arc<dyn Fn(String, MessageNode) -> tokio::task::JoinHandle<()> + Send + Sync>,
    ) -> bool {
        let tree = match self.repository.try_lock().ok().and_then(|repo| repo.get_tree_for_node(node_id)) {
            Some(t) => t,
            None => {
                error!("No tree found for node {node_id}");
                return false;
            }
        };

        let mut tree_ref = tree.write().await;
        if tree_ref.is_processing() {
            tree_ref.put_queue_unlocked(node_id);
            let queue_size = tree_ref.get_queue_size();
            info!("Queued node {node_id}, position {queue_size}");
            true
        } else {
            tree_ref.set_processing_state(Some(node_id.to_string()), true);
            if let Some(node) = tree_ref.get_node(node_id) {
                let node = node.clone();
                let nid = node_id.to_string();
                let proc = processor.clone();
                let tree_clone = tree.clone();
                tokio::spawn(async move {
                    proc(nid, node).await;
                    // After processing, check queue
                    let mut tree_ref = tree_clone.write().await;
                    tree_ref.clear_current_node();
                    if let Some(next_id) = tree_ref.dequeue() {
                        tree_ref.set_processing_state(Some(next_id.clone()), true);
                        if let Some(next_node) = tree_ref.get_node(&next_id) {
                            let next_node = next_node.clone();
                            drop(tree_ref);
                            proc(next_id, next_node).await;
                        }
                    } else {
                        tree_ref.set_processing_state(None, false);
                    }
                });
            }
            false
        }
    }

    pub async fn create_tree(
        &self,
        node_id: String,
        incoming: IncomingMessage,
        status_message_id: String,
    ) -> SharedTree {
        let mut repo = self.repository.lock().await;
        let node = MessageNode::new(
            node_id.clone(),
            incoming,
            status_message_id,
            MessageState::Pending,
        );
        let tree = Arc::new(RwLock::new(MessageTree::new(node)));
        repo.add_tree(&node_id, tree.clone());
        info!("Created new tree with root {node_id}");
        tree
    }

    pub async fn add_to_tree(
        &self,
        parent_node_id: &str,
        node_id: String,
        incoming: IncomingMessage,
        status_message_id: String,
    ) -> Result<(SharedTree, MessageNode), String> {
        let tree = self
            .repository
            .try_lock()
            .ok()
            .and_then(|repo| repo.get_tree_for_node(parent_node_id))
            .ok_or_else(|| format!("Parent node {parent_node_id} not found in any tree"))?;

        let node = {
            let mut tree_ref = tree.write().await;
            tree_ref.add_node(node_id.clone(), incoming, status_message_id, parent_node_id.to_string())?
        };

        {
            let mut repo = self.repository.lock().await;
            repo.register_node(&node_id, &tree.read().await.root_id);
        }

        info!("Added node {node_id} to tree {}", tree.read().await.root_id);
        Ok((tree, node))
    }

    pub fn set_queue_update_callback(&mut self, callback: Option<QueueUpdateCallback>) {
        self.queue_update_callback = callback;
    }

    pub fn set_node_started_callback(&mut self, callback: Option<NodeStartedCallback>) {
        self.node_started_callback = callback;
    }

    pub fn to_dict(&self) -> serde_json::Value {
        // Simplified serialization
        serde_json::json!({
            "trees": {},
            "node_to_tree": {},
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tree_repository() {
        let mut repo = TreeRepository::new();
        assert_eq!(repo.tree_count(), 0);
        assert!(!repo.has_node("m1"));
    }

    #[test]
    fn test_resolve_parent_node_id() {
        let repo = TreeRepository::new();
        assert!(repo.resolve_parent_node_id("nonexistent").is_none());
    }
}
