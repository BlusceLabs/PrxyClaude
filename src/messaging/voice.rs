use std::collections::HashMap;

use tokio::sync::Mutex;

/// Track voice notes that are still waiting on transcription.
pub struct PendingVoiceRegistry {
    pending: Mutex<HashMap<(String, String), (String, String)>>,
}

impl PendingVoiceRegistry {
    pub fn new() -> Self {
        Self {
            pending: Mutex::new(HashMap::new()),
        }
    }

    pub async fn register(&self, chat_id: &str, voice_msg_id: &str, status_msg_id: &str) {
        let mut pending = self.pending.lock().await;
        let entry = (voice_msg_id.to_string(), status_msg_id.to_string());
        pending.insert((chat_id.to_string(), voice_msg_id.to_string()), entry.clone());
        pending.insert((chat_id.to_string(), status_msg_id.to_string()), entry);
    }

    pub async fn cancel(
        &self,
        chat_id: &str,
        reply_id: &str,
    ) -> Option<(String, String)> {
        let mut pending = self.pending.lock().await;
        let entry = pending.remove(&(chat_id.to_string(), reply_id.to_string()))?;
        pending.remove(&(chat_id.to_string(), entry.0.clone()));
        pending.remove(&(chat_id.to_string(), entry.1.clone()));
        Some(entry)
    }

    pub async fn is_pending(&self, chat_id: &str, voice_msg_id: &str) -> bool {
        let pending = self.pending.lock().await;
        pending.contains_key(&(chat_id.to_string(), voice_msg_id.to_string()))
    }

    pub async fn complete(&self, chat_id: &str, voice_msg_id: &str, status_msg_id: &str) {
        let mut pending = self.pending.lock().await;
        pending.remove(&(chat_id.to_string(), voice_msg_id.to_string()));
        pending.remove(&(chat_id.to_string(), status_msg_id.to_string()));
    }
}

/// Run configured transcription backends off the event loop.
pub struct VoiceTranscriptionService {
    hf_token: String,
    nvidia_nim_api_key: String,
}

impl VoiceTranscriptionService {
    pub fn new(hf_token: String, nvidia_nim_api_key: String) -> Self {
        Self {
            hf_token,
            nvidia_nim_api_key,
        }
    }

    pub async fn transcribe(
        &self,
        _file_path: &std::path::Path,
        _mime_type: &str,
        _whisper_model: &str,
        _whisper_device: &str,
    ) -> Result<String, String> {
        // Placeholder - in production, call the transcription backend
        Err("Transcription not yet implemented in Rust".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_pending_voice_registry() {
        let registry = PendingVoiceRegistry::new();
        registry.register("chat1", "voice1", "status1").await;
        assert!(registry.is_pending("chat1", "voice1").await);

        let cancelled = registry.cancel("chat1", "voice1").await;
        assert!(cancelled.is_some());
        assert!(!registry.is_pending("chat1", "voice1").await);
    }

    #[tokio::test]
    async fn test_pending_voice_complete() {
        let registry = PendingVoiceRegistry::new();
        registry.register("chat1", "voice1", "status1").await;
        registry.complete("chat1", "voice1", "status1").await;
        assert!(!registry.is_pending("chat1", "voice1").await);
    }
}
