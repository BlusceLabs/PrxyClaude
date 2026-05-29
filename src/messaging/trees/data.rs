use std::collections::{HashMap, VecDeque};
use std::sync::Arc;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

use crate::messaging::models::IncomingMessage;

/// State of a message node in the tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MessageState {
    Pending,
    InProgress,
    Completed,
    Error,
}

impl MessageState {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::InProgress => "in_progress",
            Self::Completed => "completed",
            Self::Error => "error",
        }
    }
}

impl std::fmt::Display for MessageState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// A node in the message tree.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageNode {
    pub node_id: String,
    pub incoming: IncomingMessage,
    pub status_message_id: String,
    pub state: MessageState,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(default)]
    pub children_ids: Vec<String>,
    pub created_at: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,
}

impl MessageNode {
    pub fn new(
        node_id: String,
        incoming: IncomingMessage,
        status_message_id: String,
        state: MessageState,
    ) -> Self {
        Self {
            node_id,
            incoming,
            status_message_id,
            state,
            parent_id: None,
            session_id: None,
            children_ids: Vec::new(),
            created_at: Utc::now(),
            completed_at: None,
            error_message: None,
        }
    }

    pub fn set_error(&mut self, message: &str) {
        self.state = MessageState::Error;
        self.error_message = Some(message.to_string());
        self.completed_at = Some(Utc::now());
    }
}

/// Internal queue with snapshot/remove helpers.
#[derive(Debug, Default)]
struct SnapshotQueue {
    deque: VecDeque<String>,
    set: std::collections::HashSet<String>,
}

impl SnapshotQueue {
    fn put(&mut self, item: String) {
        self.deque.push_back(item.clone());
        self.set.insert(item);
    }

    fn get_nowait(&mut self) -> Option<String> {
        let item = self.deque.pop_front()?;
        self.set.remove(&item);
        Some(item)
    }

    fn qsize(&self) -> usize {
        self.deque.len()
    }

    fn get_snapshot(&self) -> Vec<String> {
        self.deque.iter().cloned().collect()
    }

    fn remove_if_present(&mut self, item: &str) -> bool {
        if !self.set.remove(item) {
            return false;
        }
        self.deque.retain(|x| x != item);
        true
    }
}

/// A tree of message nodes with queue functionality.
#[derive(Debug)]
pub struct MessageTree {
    pub root_id: String,
    nodes: HashMap<String, MessageNode>,
    status_to_node: HashMap<String, String>,
    queue: SnapshotQueue,
    is_processing: bool,
    current_node_id: Option<String>,
}

impl MessageTree {
    pub fn new(root_node: MessageNode) -> Self {
        let root_id = root_node.node_id.clone();
        let status_id = root_node.status_message_id.clone();
        let mut nodes = HashMap::new();
        nodes.insert(root_id.clone(), root_node);
        let mut status_to_node = HashMap::new();
        status_to_node.insert(status_id, root_id.clone());

        Self {
            root_id,
            nodes,
            status_to_node,
            queue: SnapshotQueue::default(),
            is_processing: false,
            current_node_id: None,
        }
    }

    pub fn is_processing(&self) -> bool {
        self.is_processing
    }

    pub fn get_node(&self, node_id: &str) -> Option<&MessageNode> {
        self.nodes.get(node_id)
    }

    pub fn get_node_mut(&mut self, node_id: &str) -> Option<&mut MessageNode> {
        self.nodes.get_mut(node_id)
    }

    pub fn get_root(&self) -> &MessageNode {
        &self.nodes[&self.root_id]
    }

    pub fn has_node(&self, node_id: &str) -> bool {
        self.nodes.contains_key(node_id)
    }

    pub fn all_nodes(&self) -> Vec<&MessageNode> {
        self.nodes.values().collect()
    }

    pub fn all_nodes_mut(&mut self) -> Vec<&mut MessageNode> {
        self.nodes.values_mut().collect()
    }

    pub fn find_node_by_status_message(&self, status_msg_id: &str) -> Option<&MessageNode> {
        let node_id = self.status_to_node.get(status_msg_id)?;
        self.nodes.get(node_id)
    }

    pub fn get_parent(&self, node_id: &str) -> Option<&MessageNode> {
        let parent_id = self.nodes.get(node_id)?.parent_id.as_ref()?;
        self.nodes.get(parent_id)
    }

    pub fn get_parent_session_id(&self, node_id: &str) -> Option<String> {
        let parent = self.get_parent(node_id)?;
        parent.session_id.clone()
    }

    pub fn get_children(&self, node_id: &str) -> Vec<&MessageNode> {
        match self.nodes.get(node_id) {
            Some(node) => node
                .children_ids
                .iter()
                .filter_map(|cid| self.nodes.get(cid))
                .collect(),
            None => Vec::new(),
        }
    }

    pub fn add_node(
        &mut self,
        node_id: String,
        incoming: IncomingMessage,
        status_message_id: String,
        parent_id: String,
    ) -> Result<MessageNode, String> {
        if !self.nodes.contains_key(&parent_id) {
            return Err(format!("Parent node {parent_id} not found in tree"));
        }

        let mut node = MessageNode::new(node_id.clone(), incoming, status_message_id.clone(), MessageState::Pending);
        node.parent_id = Some(parent_id.clone());

        self.nodes.insert(node_id.clone(), node.clone());
        self.status_to_node.insert(status_message_id, node_id.clone());
        if let Some(parent) = self.nodes.get_mut(&parent_id) {
            parent.children_ids.push(node_id);
        }

        Ok(node)
    }

    pub fn update_state(
        &mut self,
        node_id: &str,
        state: MessageState,
        session_id: Option<String>,
        error_message: Option<String>,
    ) {
        if let Some(node) = self.nodes.get_mut(node_id) {
            node.state = state;
            if let Some(sid) = session_id {
                node.session_id = Some(sid);
            }
            if let Some(em) = error_message {
                node.error_message = Some(em);
            }
            if state == MessageState::Completed || state == MessageState::Error {
                node.completed_at = Some(Utc::now());
            }
        }
    }

    pub fn enqueue(&mut self, node_id: &str) -> usize {
        self.queue.put(node_id.to_string());
        self.queue.qsize()
    }

    pub fn dequeue(&mut self) -> Option<String> {
        self.queue.get_nowait()
    }

    pub fn get_queue_snapshot(&self) -> Vec<String> {
        self.queue.get_snapshot()
    }

    pub fn get_queue_size(&self) -> usize {
        self.queue.qsize()
    }

    pub fn remove_from_queue(&mut self, node_id: &str) -> bool {
        self.queue.remove_if_present(node_id)
    }

    pub fn set_processing_state(&mut self, node_id: Option<String>, is_processing: bool) {
        self.is_processing = is_processing;
        self.current_node_id = if is_processing { node_id } else { None };
    }

    pub fn clear_current_node(&mut self) {
        self.current_node_id = None;
    }

    pub fn is_current_node(&self, node_id: &str) -> bool {
        self.current_node_id.as_deref() == Some(node_id)
    }

    pub fn put_queue_unlocked(&mut self, node_id: &str) {
        self.queue.put(node_id.to_string());
    }

    pub fn set_node_error_sync(&mut self, node: &mut MessageNode, error_message: &str) {
        node.state = MessageState::Error;
        node.error_message = Some(error_message.to_string());
        node.completed_at = Some(Utc::now());
    }

    pub fn drain_queue_and_cancelled(&mut self, error_message: &str) -> Vec<MessageNode> {
        let mut nodes = Vec::new();
        while let Some(node_id) = self.queue.get_nowait() {
            if let Some(node) = self.nodes.get_mut(&node_id) {
                node.state = MessageState::Error;
                node.error_message = Some(error_message.to_string());
                node.completed_at = Some(Utc::now());
                nodes.push(node.clone());
            }
        }
        nodes
    }

    pub fn reset_processing_state(&mut self) {
        self.is_processing = false;
        self.current_node_id = None;
    }

    pub fn get_descendants(&self, node_id: &str) -> Vec<String> {
        if !self.nodes.contains_key(node_id) {
            return Vec::new();
        }
        let mut result = Vec::new();
        let mut stack = vec![node_id.to_string()];
        while let Some(nid) = stack.pop() {
            result.push(nid.clone());
            if let Some(node) = self.nodes.get(&nid) {
                for cid in &node.children_ids {
                    stack.push(cid.clone());
                }
            }
        }
        result
    }

    pub fn remove_branch(&mut self, branch_root_id: &str) -> Vec<MessageNode> {
        if !self.nodes.contains_key(branch_root_id) {
            return Vec::new();
        }

        let descendants = self.get_descendants(branch_root_id);
        let mut removed = Vec::new();

        // Get parent before removing
        let parent_id = self.nodes
            .get(branch_root_id)
            .and_then(|n| n.parent_id.clone());

        for nid in &descendants {
            if let Some(node) = self.nodes.remove(nid) {
                self.status_to_node.remove(&node.status_message_id);
                removed.push(node);
            }
        }

        // Update parent's children list
        if let Some(parent_id) = parent_id {
            if let Some(parent) = self.nodes.get_mut(&parent_id) {
                parent.children_ids.retain(|c| c != branch_root_id);
            }
        }

        removed
    }

    pub fn to_dict(&self) -> serde_json::Value {
        let nodes: HashMap<&str, &MessageNode> = self
            .nodes
            .iter()
            .map(|(k, v)| (k.as_str(), v))
            .collect();
        serde_json::json!({
            "root_id": self.root_id,
            "nodes": nodes,
        })
    }

    pub fn from_dict(data: &serde_json::Value) -> Option<Self> {
        let root_id = data.get("root_id")?.as_str()?.to_string();
        let nodes_data = data.get("nodes")?;

        let root_node_data = nodes_data.get(&root_id)?;
        let root_node = serde_json::from_value::<MessageNode>(root_node_data.clone()).ok()?;

        let mut tree = Self::new(root_node);

        if let Some(obj) = nodes_data.as_object() {
            for (node_id, node_data) in obj {
                if node_id == &root_id {
                    continue;
                }
                if let Ok(node) = serde_json::from_value::<MessageNode>(node_data.clone()) {
                    let nid = node.node_id.clone();
                    let sid = node.status_message_id.clone();
                    tree.nodes.insert(nid.clone(), node);
                    tree.status_to_node.insert(sid, nid);
                }
            }
        }

        Some(tree)
    }
}

/// Thread-safe tree wrapped in Arc<RwLock<>>.
pub type SharedTree = Arc<RwLock<MessageTree>>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::messaging::models::IncomingMessage;

    fn make_incoming(text: &str) -> IncomingMessage {
        IncomingMessage {
            text: text.into(),
            chat_id: "c1".into(),
            user_id: "u1".into(),
            message_id: "m1".into(),
            platform: "test".into(),
            reply_to_message_id: None,
            message_thread_id: None,
            username: None,
            status_message_id: None,
            timestamp: Utc::now(),
        }
    }

    #[test]
    fn test_create_tree() {
        let root = MessageNode::new(
            "m1".into(),
            make_incoming("hello"),
            "s1".into(),
            MessageState::Pending,
        );
        let tree = MessageTree::new(root);
        assert_eq!(tree.root_id, "m1");
        assert!(tree.has_node("m1"));
    }

    #[test]
    fn test_add_node() {
        let root = MessageNode::new(
            "m1".into(),
            make_incoming("hello"),
            "s1".into(),
            MessageState::Pending,
        );
        let mut tree = MessageTree::new(root);
        let result = tree.add_node(
            "m2".into(),
            make_incoming("world"),
            "s2".into(),
            "m1".into(),
        );
        assert!(result.is_ok());
        assert!(tree.has_node("m2"));
        assert_eq!(tree.get_node("m2").unwrap().parent_id.as_deref(), Some("m1"));
    }

    #[test]
    fn test_queue_operations() {
        let root = MessageNode::new(
            "m1".into(),
            make_incoming("hello"),
            "s1".into(),
            MessageState::Pending,
        );
        let mut tree = MessageTree::new(root);
        tree.enqueue("m1");
        assert_eq!(tree.get_queue_size(), 1);
        assert_eq!(tree.dequeue(), Some("m1".into()));
        assert_eq!(tree.get_queue_size(), 0);
    }

    #[test]
    fn test_remove_branch() {
        let root = MessageNode::new(
            "m1".into(),
            make_incoming("hello"),
            "s1".into(),
            MessageState::Pending,
        );
        let mut tree = MessageTree::new(root);
        tree.add_node("m2".into(), make_incoming("a"), "s2".into(), "m1".into()).unwrap();
        tree.add_node("m3".into(), make_incoming("b"), "s3".into(), "m2".into()).unwrap();
        let removed = tree.remove_branch("m2");
        assert_eq!(removed.len(), 2);
        assert!(!tree.has_node("m2"));
        assert!(!tree.has_node("m3"));
        assert!(tree.has_node("m1"));
    }

    #[test]
    fn test_serialization() {
        let root = MessageNode::new(
            "m1".into(),
            make_incoming("hello"),
            "s1".into(),
            MessageState::Pending,
        );
        let tree = MessageTree::new(root);
        let dict = tree.to_dict();
        let restored = MessageTree::from_dict(&dict);
        assert!(restored.is_some());
        assert_eq!(restored.unwrap().root_id, "m1");
    }
}
