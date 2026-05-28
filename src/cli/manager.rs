use std::collections::HashMap;
use tokio::sync::Mutex;
use uuid::Uuid;

use super::session::CLISession;

pub struct CLISessionManager {
    workspace: String,
    api_url: String,
    allowed_dirs: Vec<String>,
    plans_directory: Option<String>,
    claude_bin: String,
    log_raw_cli_diagnostics: bool,

    sessions: Mutex<HashMap<String, CLISession>>,
    pending_sessions: Mutex<HashMap<String, CLISession>>,
    temp_to_real: Mutex<HashMap<String, String>>,
    real_to_temp: Mutex<HashMap<String, String>>,
}

impl CLISessionManager {
    pub fn new(
        workspace_path: String,
        api_url: String,
        allowed_dirs: Option<Vec<String>>,
        plans_directory: Option<String>,
        claude_bin: String,
        log_raw_cli_diagnostics: bool,
    ) -> Self {
        Self {
            workspace: workspace_path,
            api_url,
            allowed_dirs: allowed_dirs.unwrap_or_default(),
            plans_directory,
            claude_bin,
            log_raw_cli_diagnostics,
            sessions: Mutex::new(HashMap::new()),
            pending_sessions: Mutex::new(HashMap::new()),
            temp_to_real: Mutex::new(HashMap::new()),
            real_to_temp: Mutex::new(HashMap::new()),
        }
    }

    pub async fn get_or_create_session(
        &self,
        session_id: Option<&str>,
    ) -> (String, bool) {
        if let Some(sid) = session_id {
            let lookup_id = {
                let temp_to_real = self.temp_to_real.lock().await;
                temp_to_real.get(sid).cloned().unwrap_or_else(|| sid.to_string())
            };

            {
                let sessions = self.sessions.lock().await;
                if sessions.contains_key(&lookup_id) {
                    return (lookup_id, false);
                }
            }
            {
                let pending = self.pending_sessions.lock().await;
                if pending.contains_key(&lookup_id) {
                    return (lookup_id, false);
                }
            }
        }

        let temp_id = session_id
            .map(|s| s.to_string())
            .unwrap_or_else(|| format!("pending_{}", &Uuid::new_v4().to_string()[..8]));

        let new_session = CLISession::new(
            self.workspace.clone(),
            self.api_url.clone(),
            Some(self.allowed_dirs.clone()),
            self.plans_directory.clone(),
            self.claude_bin.clone(),
            self.log_raw_cli_diagnostics,
        );

        self.pending_sessions
            .lock()
            .await
            .insert(temp_id.clone(), new_session);

        tracing::info!("Created new session: {}", temp_id);
        (temp_id, true)
    }

    pub async fn register_real_session_id(
        &self,
        temp_id: &str,
        real_session_id: &str,
    ) -> bool {
        let mut pending = self.pending_sessions.lock().await;
        let session = match pending.remove(temp_id) {
            Some(s) => s,
            None => {
                tracing::warn!("Temp session {} not found", temp_id);
                return false;
            }
        };

        self.sessions
            .lock()
            .await
            .insert(real_session_id.to_string(), session);
        self.temp_to_real
            .lock()
            .await
            .insert(temp_id.to_string(), real_session_id.to_string());
        self.real_to_temp
            .lock()
            .await
            .insert(real_session_id.to_string(), temp_id.to_string());

        tracing::info!("Registered session: {} -> {}", temp_id, real_session_id);
        true
    }

    pub async fn remove_session(&self, session_id: &str) -> bool {
        {
            let mut pending = self.pending_sessions.lock().await;
            if let Some(mut session) = pending.remove(session_id) {
                session.stop().await;
                return true;
            }
        }

        {
            let mut sessions = self.sessions.lock().await;
            if let Some(mut session) = sessions.remove(session_id) {
                session.stop().await;
                let mut real_to_temp = self.real_to_temp.lock().await;
                if let Some(temp_id) = real_to_temp.remove(session_id) {
                    self.temp_to_real.lock().await.remove(&temp_id);
                }
                return true;
            }
        }

        false
    }

    pub async fn stop_all(&self) {
        let mut all_sessions = Vec::new();
        {
            let mut sessions = self.sessions.lock().await;
            for (_id, session) in sessions.drain() {
                all_sessions.push(session);
            }
        }
        {
            let mut pending = self.pending_sessions.lock().await;
            for (_id, session) in pending.drain() {
                all_sessions.push(session);
            }
        }

        for mut session in all_sessions {
            session.stop().await;
        }

        self.temp_to_real.lock().await.clear();
        self.real_to_temp.lock().await.clear();
        tracing::info!("All sessions stopped");
    }

    pub async fn get_stats(&self) -> HashMap<String, usize> {
        let active = self.sessions.lock().await.len();
        let pending = self.pending_sessions.lock().await.len();
        let mut stats = HashMap::new();
        stats.insert("active_sessions".to_string(), active);
        stats.insert("pending_sessions".to_string(), pending);
        stats
    }
}
