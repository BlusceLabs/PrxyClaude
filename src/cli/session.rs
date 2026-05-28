use std::collections::HashMap;
use tokio::process::{Child, Command};
use tokio::sync::Mutex;

use super::process_registry::{register_pid, unregister_pid};

const MAX_STDERR_CAPTURE_BYTES: usize = 256 * 1024;

#[derive(Debug, Clone)]
pub struct ClaudeCliConfig {
    pub workspace_path: String,
    pub api_url: String,
    pub allowed_dirs: Vec<String>,
    pub plans_directory: Option<String>,
    pub claude_bin: String,
}

pub struct CLISession {
    pub config: ClaudeCliConfig,
    pub workspace: String,
    pub api_url: String,
    pub allowed_dirs: Vec<String>,
    pub plans_directory: Option<String>,
    pub claude_bin: String,
    log_raw_cli_diagnostics: bool,
    process: Option<Child>,
    pub current_session_id: Option<String>,
    is_busy: Mutex<bool>,
    cli_lock: Mutex<()>,
}

impl CLISession {
    pub fn new(
        workspace_path: String,
        api_url: String,
        allowed_dirs: Option<Vec<String>>,
        plans_directory: Option<String>,
        claude_bin: String,
        log_raw_cli_diagnostics: bool,
    ) -> Self {
        let workspace = std::path::Path::new(&workspace_path)
            .canonicalize()
            .unwrap_or_else(|_| std::path::PathBuf::from(&workspace_path))
            .to_string_lossy()
            .to_string();

        let allowed_dirs: Vec<String> = allowed_dirs.unwrap_or_default();
        let allowed_dirs: Vec<String> = allowed_dirs
            .into_iter()
            .map(|d| {
                std::path::Path::new(&d)
                    .canonicalize()
                    .unwrap_or_else(|_| std::path::PathBuf::from(&d))
                    .to_string_lossy()
                    .to_string()
            })
            .collect();

        let config = ClaudeCliConfig {
            workspace_path: workspace.clone(),
            api_url: api_url.clone(),
            allowed_dirs: allowed_dirs.clone(),
            plans_directory: plans_directory.clone(),
            claude_bin: claude_bin.clone(),
        };

        Self {
            config,
            workspace,
            api_url,
            allowed_dirs,
            plans_directory,
            claude_bin,
            log_raw_cli_diagnostics,
            process: None,
            current_session_id: None,
            is_busy: Mutex::new(false),
            cli_lock: Mutex::new(()),
        }
    }

    pub async fn is_busy(&self) -> bool {
        *self.is_busy.lock().await
    }

    pub async fn start_task(
        &mut self,
        prompt: &str,
        session_id: Option<&str>,
        fork_session: bool,
    ) -> Result<Vec<HashMap<String, serde_json::Value>>, String> {
        let _lock = self.cli_lock.lock().await;
        *self.is_busy.lock().await = true;

        let mut cmd = Command::new(&self.claude_bin);
        cmd.env("ANTHROPIC_API_KEY", "sk-placeholder-key-for-proxy");

        let base_url = if self.api_url.ends_with("/v1") {
            self.api_url.trim_end_matches("/v1")
        } else {
            &self.api_url
        };
        cmd.env("ANTHROPIC_BASE_URL", base_url);
        cmd.env("ANTHROPIC_API_URL", &self.api_url);
        cmd.env("TERM", "dumb");
        cmd.env("PYTHONIOENCODING", "utf-8");

        if let Some(sid) = session_id {
            if !sid.starts_with("pending_") {
                cmd.arg("--resume");
                cmd.arg(sid);
                if fork_session {
                    cmd.arg("--fork-session");
                }
            }
        }

        cmd.arg("-p");
        cmd.arg(prompt);
        cmd.arg("--output-format");
        cmd.arg("stream-json");
        cmd.arg("--dangerously-skip-permissions");
        cmd.arg("--verbose");

        for d in &self.allowed_dirs {
            cmd.arg("--add-dir");
            cmd.arg(d);
        }

        if let Some(ref plans_dir) = self.plans_directory {
            let settings = serde_json::json!({"plansDirectory": plans_dir});
            cmd.arg("--settings");
            cmd.arg(settings.to_string());
        }

        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());
        cmd.current_dir(&self.workspace);

        let mut child = cmd.spawn().map_err(|e| format!("Failed to spawn CLI: {}", e))?;

        if let Some(pid) = child.id() {
            register_pid(pid);
        }

        let stdout = child.stdout.take().ok_or("No stdout")?;
        let stderr = child.stderr.take().ok_or("No stderr")?;

        let mut reader = tokio::io::BufReader::new(stdout);
        let mut stderr_reader = tokio::io::BufReader::new(stderr);

        let stderr_handle = tokio::spawn(async move {
            let mut received = 0usize;
            let mut parts = Vec::new();
            use tokio::io::AsyncReadExt;
            let mut buf = [0u8; 65536];
            loop {
                let n = stderr_reader.read(&mut buf).await.unwrap_or(0);
                if n == 0 {
                    break;
                }
                if received < MAX_STDERR_CAPTURE_BYTES {
                    let take = (n).min(MAX_STDERR_CAPTURE_BYTES - received);
                    parts.extend_from_slice(&buf[..take]);
                    received += take;
                }
            }
            parts
        });

        use tokio::io::AsyncBufReadExt;
        let mut line = String::new();
        let mut events = Vec::new();
        let mut session_id_extracted = false;

        loop {
            line.clear();
            match reader.read_line(&mut line).await {
                Ok(0) => break,
                Ok(_) => {
                    let line_str = line.trim().to_string();
                    if line_str.is_empty() {
                        continue;
                    }
                    if let Ok(value) =
                        serde_json::from_str::<HashMap<String, serde_json::Value>>(&line_str)
                    {
                        if !session_id_extracted {
                            if let Some(sid) = extract_session_id(&value) {
                                self.current_session_id = Some(sid.clone());
                                session_id_extracted = true;
                                let mut event = HashMap::new();
                                event.insert(
                                    "type".to_string(),
                                    serde_json::Value::String("session_info".to_string()),
                                );
                                event.insert(
                                    "session_id".to_string(),
                                    serde_json::Value::String(sid),
                                );
                                events.push(event);
                            }
                        }
                        events.push(value);
                    } else {
                        if self.log_raw_cli_diagnostics {
                            tracing::debug!("Non-JSON output: {}", line_str);
                        } else {
                            tracing::debug!("Non-JSON CLI line: char_len={}", line_str.len());
                        }
                        let mut event = HashMap::new();
                        event.insert(
                            "type".to_string(),
                            serde_json::Value::String("raw".to_string()),
                        );
                        event.insert(
                            "content".to_string(),
                            serde_json::Value::String(line_str),
                        );
                        events.push(event);
                    }
                }
                Err(e) => {
                    let mut event = HashMap::new();
                    event.insert(
                        "type".to_string(),
                        serde_json::Value::String("error".to_string()),
                    );
                    event.insert(
                        "error".to_string(),
                        serde_json::Value::String(format!("Read error: {}", e)),
                    );
                    events.push(event);
                    break;
                }
            }
        }

        let stderr_bytes = stderr_handle.await.unwrap_or_default();
        let stderr_text = if !stderr_bytes.is_empty() {
            Some(String::from_utf8_lossy(&stderr_bytes).trim().to_string())
        } else {
            None
        };

        if let Some(ref stderr) = stderr_text {
            if !stderr.is_empty() {
                if self.log_raw_cli_diagnostics {
                    tracing::error!("Claude CLI stderr: {}", stderr);
                } else {
                    tracing::error!(
                        "Claude CLI stderr: bytes={} text_chars={}",
                        stderr_bytes.len(),
                        stderr.len()
                    );
                }
                let mut event = HashMap::new();
                event.insert(
                    "type".to_string(),
                    serde_json::Value::String("error".to_string()),
                );
                event.insert(
                    "error".to_string(),
                    serde_json::json!({"message": stderr}),
                );
                events.push(event);
            }
        }

        let status = child.wait().await.map_err(|e| format!("Wait failed: {}", e))?;
        let code = status.code().unwrap_or(-1);

        if let Some(pid) = child.id() {
            unregister_pid(pid);
        }

        let mut event = HashMap::new();
        event.insert(
            "type".to_string(),
            serde_json::Value::String("exit".to_string()),
        );
        event.insert(
            "code".to_string(),
            serde_json::Value::Number(serde_json::Number::from(code)),
        );
        if let Some(ref stderr) = stderr_text {
            event.insert(
                "stderr".to_string(),
                serde_json::Value::String(stderr.clone()),
            );
        }
        events.push(event);

        *self.is_busy.lock().await = false;
        self.process = None;

        Ok(events)
    }

    pub async fn stop(&mut self) -> bool {
        if let Some(ref mut child) = self.process {
            if let Some(pid) = child.id() {
                tracing::info!("Stopping Claude CLI process {}", pid);
            }
            let _ = child.start_kill();
            match tokio::time::timeout(std::time::Duration::from_secs(5), child.wait()).await {
                Ok(Ok(_)) => {}
                _ => {
                    let _ = child.kill().await;
                    let _ = child.wait().await;
                }
            }
            if let Some(pid) = child.id() {
                unregister_pid(pid);
            }
            self.process = None;
            return true;
        }
        false
    }
}

fn extract_session_id(event: &HashMap<String, serde_json::Value>) -> Option<String> {
    if let Some(sid) = event.get("session_id").and_then(|v| v.as_str()) {
        return Some(sid.to_string());
    }
    if let Some(sid) = event.get("sessionId").and_then(|v| v.as_str()) {
        return Some(sid.to_string());
    }

    for key in &["init", "system", "result", "metadata"] {
        if let Some(nested) = event.get(*key).and_then(|v| v.as_object()) {
            if let Some(sid) = nested.get("session_id").and_then(|v| v.as_str()) {
                return Some(sid.to_string());
            }
            if let Some(sid) = nested.get("sessionId").and_then(|v| v.as_str()) {
                return Some(sid.to_string());
            }
        }
    }

    if let Some(conv) = event.get("conversation").and_then(|v| v.as_object()) {
        if let Some(cid) = conv.get("id").and_then(|v| v.as_str()) {
            return Some(cid.to_string());
        }
    }

    None
}
