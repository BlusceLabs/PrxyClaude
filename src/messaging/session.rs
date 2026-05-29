use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use chrono::Utc;
use serde::{Deserialize, Serialize};
use tokio::sync::{Mutex, RwLock};
use tracing::{debug, error, info};


/// Record of a message for best-effort deletion.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageRecord {
    pub message_id: String,
    pub ts: String,
    pub direction: String,
    pub kind: String,
}

/// Persistent storage for message ↔ Claude session mappings and message trees.
pub struct SessionStore {
    storage_path: PathBuf,
    trees: Arc<RwLock<HashMap<String, serde_json::Value>>>,
    node_to_tree: Arc<RwLock<HashMap<String, String>>>,
    message_log: Arc<RwLock<HashMap<String, Vec<MessageRecord>>>>,
    message_log_ids: Arc<RwLock<HashMap<String, HashSet<String>>>>,
    dirty: Arc<Mutex<bool>>,
    message_log_cap: Option<usize>,
}

impl SessionStore {
    pub fn new(storage_path: impl AsRef<Path>, message_log_cap: Option<usize>) -> Self {
        let path = storage_path.as_ref().to_path_buf();
        let mut store = Self {
            storage_path: path.clone(),
            trees: Arc::new(RwLock::new(HashMap::new())),
            node_to_tree: Arc::new(RwLock::new(HashMap::new())),
            message_log: Arc::new(RwLock::new(HashMap::new())),
            message_log_ids: Arc::new(RwLock::new(HashMap::new())),
            dirty: Arc::new(Mutex::new(false)),
            message_log_cap,
        };
        store.load();
        store
    }

    fn make_chat_key(platform: &str, chat_id: &str) -> String {
        format!("{platform}:{chat_id}")
    }

    fn load(&mut self) {
        if !self.storage_path.exists() {
            return;
        }

        let data = match fs::read_to_string(&self.storage_path) {
            Ok(d) => d,
            Err(e) => {
                error!("Failed to read sessions file: {e}");
                return;
            }
        };

        let parsed: serde_json::Value = match serde_json::from_str(&data) {
            Ok(v) => v,
            Err(e) => {
                error!("Failed to parse sessions file: {e}");
                return;
            }
        };

        // Load trees
        if let Some(trees_obj) = parsed.get("trees").and_then(|v| v.as_object()) {
            let mut trees = HashMap::new();
            for (k, v) in trees_obj {
                trees.insert(k.clone(), v.clone());
            }
            // We store as serde_json::Value for now; in production deserialize to MessageTree
            *self.trees.blocking_write() = trees;
        }

        // Load node_to_tree
        if let Some(ntt) = parsed.get("node_to_tree").and_then(|v| v.as_object()) {
            let mut map = HashMap::new();
            for (k, v) in ntt {
                if let Some(s) = v.as_str() {
                    map.insert(k.clone(), s.to_string());
                }
            }
            *self.node_to_tree.blocking_write() = map;
        }

        // Load message_log
        if let Some(raw_log) = parsed.get("message_log").and_then(|v| v.as_object()) {
            let mut log = HashMap::new();
            let mut ids = HashMap::new();
            for (chat_key, items) in raw_log {
                if let Some(arr) = items.as_array() {
                    let mut records = Vec::new();
                    let mut seen = HashSet::new();
                    for item in arr {
                        if let Some(obj) = item.as_object() {
                            let mid = obj.get("message_id").and_then(|v| v.as_str()).unwrap_or("");
                            if mid.is_empty() || seen.contains(mid) {
                                continue;
                            }
                            seen.insert(mid.to_string());
                            records.push(MessageRecord {
                                message_id: mid.to_string(),
                                ts: obj.get("ts").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                                direction: obj.get("direction").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                                kind: obj.get("kind").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                            });
                        }
                    }
                    ids.insert(chat_key.clone(), seen);
                    log.insert(chat_key.clone(), records);
                }
            }
            *self.message_log.blocking_write() = log;
            *self.message_log_ids.blocking_write() = ids;
        }

        info!(
            "Loaded {} trees from {}",
            self.trees.blocking_read().len(),
            self.storage_path.display()
        );
    }

    async fn snapshot(&self) -> serde_json::Value {
        let trees = self.trees.read().await;
        let node_to_tree = self.node_to_tree.read().await;
        let message_log = self.message_log.read().await;

        serde_json::json!({
            "trees": *trees,
            "node_to_tree": *node_to_tree,
            "message_log": *message_log,
        })
    }

    async fn write_data(&self, data: &serde_json::Value) -> Result<(), std::io::Error> {
        let abs_target = std::fs::canonicalize(&self.storage_path)
            .unwrap_or_else(|_| self.storage_path.clone());
        let dir_name = abs_target.parent().unwrap_or(Path::new("."));
        let tmp_path = dir_name.join(".sessions.tmp.json");

        let json = serde_json::to_string_pretty(data).unwrap_or_default();
        fs::write(&tmp_path, &json)?;
        fs::rename(&tmp_path, &abs_target)?;
        Ok(())
    }

    pub async fn record_message_id(
        &self,
        platform: &str,
        chat_id: &str,
        message_id: &str,
        direction: &str,
        kind: &str,
    ) {
        if message_id.is_empty() {
            return;
        }

        let chat_key = Self::make_chat_key(platform, chat_id);
        let mid = message_id.to_string();

        {
            let mut seen = self.message_log_ids.write().await;
            let entry = seen.entry(chat_key.clone()).or_default();
            if entry.contains(&mid) {
                return;
            }
        }

        let rec = MessageRecord {
            message_id: mid.clone(),
            ts: Utc::now().to_rfc3339(),
            direction: direction.to_string(),
            kind: kind.to_string(),
        };

        {
            let mut log = self.message_log.write().await;
            log.entry(chat_key.clone()).or_default().push(rec);
        }

        {
            let mut seen = self.message_log_ids.write().await;
            seen.entry(chat_key.clone()).or_default().insert(mid);
        }

        // Optional cap
        if let Some(cap) = self.message_log_cap {
            if cap > 0 {
                let mut log = self.message_log.write().await;
                if let Some(items) = log.get_mut(&chat_key) {
                    if items.len() > cap {
                        let drain_count = items.len() - cap;
                        items.drain(..drain_count);
                    }
                }
            }
        }

        *self.dirty.lock().await = true;
    }

    pub async fn get_message_ids_for_chat(&self, platform: &str, chat_id: &str) -> Vec<String> {
        let chat_key = Self::make_chat_key(platform, chat_id);
        let log = self.message_log.read().await;
        log.get(&chat_key)
            .map(|items| items.iter().map(|r| r.message_id.clone()).collect())
            .unwrap_or_default()
    }

    pub async fn clear_all(&self) {
        {
            let mut trees = self.trees.write().await;
            trees.clear();
        }
        {
            let mut ntt = self.node_to_tree.write().await;
            ntt.clear();
        }
        {
            let mut log = self.message_log.write().await;
            log.clear();
        }
        {
            let mut ids = self.message_log_ids.write().await;
            ids.clear();
        }

        let snapshot = self.snapshot().await;
        if let Err(e) = self.write_data(&snapshot).await {
            error!("Failed to save sessions: {e}");
            *self.dirty.lock().await = true;
        }
    }

    // ==================== Tree Methods ====================

    pub async fn save_tree(&self, root_id: &str, tree_data: serde_json::Value) {
        {
            let mut trees = self.trees.write().await;
            trees.insert(root_id.to_string(), tree_data.clone());

            // Update node-to-tree mapping
            if let Some(nodes) = tree_data.get("nodes").and_then(|v| v.as_object()) {
                let mut ntt = self.node_to_tree.write().await;
                for node_id in nodes.keys() {
                    ntt.insert(node_id.clone(), root_id.to_string());
                }
            }
        }

        *self.dirty.lock().await = true;
        debug!("Saved tree {root_id}");
    }

    pub async fn get_tree(&self, root_id: &str) -> Option<serde_json::Value> {
        let trees = self.trees.read().await;
        trees.get(root_id).cloned()
    }

    pub async fn register_node(&self, node_id: &str, root_id: &str) {
        let mut ntt = self.node_to_tree.write().await;
        ntt.insert(node_id.to_string(), root_id.to_string());
        *self.dirty.lock().await = true;
    }

    pub async fn remove_node_mappings(&self, node_ids: &[String]) {
        let mut ntt = self.node_to_tree.write().await;
        for nid in node_ids {
            ntt.remove(nid);
        }
        *self.dirty.lock().await = true;
    }

    pub async fn remove_tree(&self, root_id: &str) {
        let tree_data = {
            let mut trees = self.trees.write().await;
            trees.remove(root_id)
        };

        if let Some(data) = tree_data {
            if let Some(nodes) = data.get("nodes").and_then(|v| v.as_object()) {
                let mut ntt = self.node_to_tree.write().await;
                for node_id in nodes.keys() {
                    ntt.remove(node_id);
                }
            }
            *self.dirty.lock().await = true;
        }
    }

    pub async fn get_all_trees(&self) -> HashMap<String, serde_json::Value> {
        self.trees.read().await.clone()
    }

    pub async fn get_node_mapping(&self) -> HashMap<String, String> {
        self.node_to_tree.read().await.clone()
    }

    pub async fn flush_pending_save(&self) {
        let snapshot = self.snapshot().await;
        if let Err(e) = self.write_data(&snapshot).await {
            error!("Failed to save sessions: {e}");
            *self.dirty.lock().await = true;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn temp_store() -> (SessionStore, PathBuf) {
        let dir = std::env::temp_dir().join(format!("session_test_{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("sessions.json");
        (SessionStore::new(&path, None), dir)
    }

    #[tokio::test]
    async fn test_record_and_get_message_ids() {
        let (store, dir) = temp_store();
        store.record_message_id("telegram", "123", "msg1", "in", "content").await;
        store.record_message_id("telegram", "123", "msg2", "out", "status").await;

        let ids = store.get_message_ids_for_chat("telegram", "123").await;
        assert_eq!(ids.len(), 2);
        assert!(ids.contains(&"msg1".to_string()));

        // Duplicate should be ignored
        store.record_message_id("telegram", "123", "msg1", "in", "content").await;
        let ids = store.get_message_ids_for_chat("telegram", "123").await;
        assert_eq!(ids.len(), 2);

        let _ = fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn test_save_and_get_tree() {
        let (store, dir) = temp_store();
        let tree_data = serde_json::json!({
            "root_id": "m1",
            "nodes": {"m1": {"node_id": "m1"}}
        });
        store.save_tree("m1", tree_data.clone()).await;

        let got = store.get_tree("m1").await;
        assert!(got.is_some());
        assert_eq!(got.unwrap()["root_id"], "m1");

        let _ = fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn test_clear_all() {
        let (store, dir) = temp_store();
        store.record_message_id("telegram", "123", "msg1", "in", "content").await;
        store.clear_all().await;

        let ids = store.get_message_ids_for_chat("telegram", "123").await;
        assert!(ids.is_empty());

        let _ = fs::remove_dir_all(&dir);
    }
}
