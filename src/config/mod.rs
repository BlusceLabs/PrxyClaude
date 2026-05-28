pub mod constants;
pub mod logging_config;
pub mod nim;
pub mod provider_catalog;

use serde::{Deserialize, Serialize};

use std::env;
use std::path::PathBuf;
use thiserror::Error;

use self::nim::NimSettings;

#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("Config file not found: {0}")]
    FileNotFound(PathBuf),
    #[error("Failed to parse config: {0}")]
    ParseError(String),
    #[error("Environment variable not found: {0}")]
    EnvVarNotFound(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub server: ServerConfig,
    pub logging: LoggingConfig,
    pub providers: ProviderConfig,
    pub features: FeatureConfig,
    pub performance: PerformanceConfig,
    pub security: SecurityConfig,
    pub messaging: MessagingConfig,
    pub http: HttpConfig,
    pub voice: VoiceConfig,
    pub bot: BotConfig,
    pub nim: NimSettings,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub addr: String,
    pub cors_enabled: bool,
    pub cors_origins: Vec<String>,
    pub log_file: Option<String>,
    pub anthropic_auth_token: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessagingConfig {
    pub platform: String,
    pub rate_limit: usize,
    pub rate_window: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpConfig {
    pub read_timeout: f64,
    pub write_timeout: f64,
    pub connect_timeout: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoiceConfig {
    pub note_enabled: bool,
    pub whisper_device: String,
    pub whisper_model: String,
    pub hf_token: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BotConfig {
    pub telegram_bot_token: Option<String>,
    pub allowed_telegram_user_id: Option<String>,
    pub discord_bot_token: Option<String>,
    pub allowed_discord_channels: Option<String>,
    pub claude_workspace: String,
    pub allowed_dir: String,
    pub claude_cli_bin: String,
    pub max_message_log_entries_per_chat: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    pub level: String,
    pub file: Option<String>,
    pub raw_api_payloads: bool,
    pub raw_sse_events: bool,
    pub raw_cli_diagnostics: bool,
    pub raw_messaging_content: bool,
    pub messaging_error_details: bool,
    pub error_tracebacks: bool,
    pub debug_platform_edits: bool,
    pub debug_subagent_stack: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub default_provider: String,
    pub openai: OpenAIConfig,
    pub anthropic: AnthropicConfig,
    pub open_router: OpenRouterConfig,
    pub nvidia_nim: NvidiaNimConfig,
    pub deepseek: SimpleProviderConfig,
    pub kimi: SimpleProviderConfig,
    pub llamacpp: SimpleProviderConfig,
    pub lmstudio: SimpleProviderConfig,
    pub fireworks: SimpleProviderConfig,
    pub siliconflow: SimpleProviderConfig,
    pub z_ai: SimpleProviderConfig,
    pub gemini: SimpleProviderConfig,
    pub ollama: SimpleProviderConfig,
    pub cloudflare_gateway: SimpleProviderConfig,
    pub model: String,
    pub model_opus: Option<String>,
    pub model_sonnet: Option<String>,
    pub model_haiku: Option<String>,
    pub enable_model_thinking: bool,
    pub enable_opus_thinking: Option<bool>,
    pub enable_sonnet_thinking: Option<bool>,
    pub enable_haiku_thinking: Option<bool>,
    pub rate_limit: usize,
    pub rate_window: usize,
    pub max_concurrency: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAIConfig {
    pub api_key: Option<String>,
    pub base_url: Option<String>,
    pub organization: Option<String>,
    pub proxy: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnthropicConfig {
    pub api_key: Option<String>,
    pub beta_features: Vec<String>,
    pub base_url: Option<String>,
    pub proxy: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenRouterConfig {
    pub api_key: Option<String>,
    pub base_url: String,
    pub referer: Option<String>,
    pub proxy: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NvidiaNimConfig {
    pub base_url: Option<String>,
    pub model_id: Option<String>,
    pub api_key: Option<String>,
    pub proxy: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SimpleProviderConfig {
    pub api_key: Option<String>,
    pub base_url: Option<String>,
    pub proxy: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureConfig {
    pub web_tools_enabled: bool,
    pub web_fetch_egress_allow_private: bool,
    pub web_fetch_egress_allowed_schemes: Vec<String>,
    pub max_web_fetch_chars: usize,
    pub max_web_search_results: usize,
    pub max_web_fetch_redirects: usize,
    pub web_fetch_redirect_response_body_cap_bytes: usize,
    pub voice_enabled: bool,
    pub voice_local_enabled: bool,
    pub fast_prefix_detection: bool,
    pub enable_network_probe_mock: bool,
    pub enable_title_generation_skip: bool,
    pub enable_suggestion_mode_skip: bool,
    pub enable_filepath_extraction_mock: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceConfig {
    pub max_concurrent_requests: usize,
    pub request_timeout_seconds: u64,
    pub streaming_timeout_seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConfig {
    pub enable_rate_limiting: bool,
    pub rate_limit_requests_per_minute: usize,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: "info".to_string(),
            file: None,
            raw_api_payloads: false,
            raw_sse_events: false,
            raw_cli_diagnostics: false,
            raw_messaging_content: false,
            messaging_error_details: false,
            error_tracebacks: false,
            debug_platform_edits: false,
            debug_subagent_stack: false,
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            server: ServerConfig {
                addr: "0.0.0.0:8082".to_string(),
                cors_enabled: true,
                cors_origins: vec![
                    "http://localhost:3000".to_string(),
                    "http://localhost:8080".to_string(),
                ],
                log_file: Some("server.log".to_string()),
                anthropic_auth_token: String::new(),
            },
            logging: LoggingConfig::default(),
            providers: ProviderConfig {
                default_provider: "open_router".to_string(),
                openai: OpenAIConfig {
                    api_key: None,
                    base_url: None,
                    organization: None,
                    proxy: None,
                },
                anthropic: AnthropicConfig {
                    api_key: None,
                    beta_features: vec![],
                    base_url: None,
                    proxy: None,
                },
                open_router: OpenRouterConfig {
                    api_key: None,
                    base_url: "https://openrouter.ai/api/v1".to_string(),
                    referer: None,
                    proxy: None,
                },
                nvidia_nim: NvidiaNimConfig {
                    base_url: None,
                    model_id: None,
                    api_key: None,
                    proxy: None,
                },
                deepseek: SimpleProviderConfig::default(),
                kimi: SimpleProviderConfig::default(),
                llamacpp: SimpleProviderConfig::default(),
                lmstudio: SimpleProviderConfig::default(),
                fireworks: SimpleProviderConfig::default(),
                siliconflow: SimpleProviderConfig::default(),
                z_ai: SimpleProviderConfig::default(),
                gemini: SimpleProviderConfig::default(),
                ollama: SimpleProviderConfig::default(),
                cloudflare_gateway: SimpleProviderConfig::default(),
                model: "nvidia_nim/z-ai/glm4.7".to_string(),
                model_opus: None,
                model_sonnet: None,
                model_haiku: None,
                enable_model_thinking: true,
                enable_opus_thinking: None,
                enable_sonnet_thinking: None,
                enable_haiku_thinking: None,
                rate_limit: 60,
                rate_window: 30,
                max_concurrency: 10,
            },
            features: FeatureConfig {
                web_tools_enabled: false,
                web_fetch_egress_allow_private: false,
                web_fetch_egress_allowed_schemes: vec!["http".to_string(), "https".to_string()],
                max_web_fetch_chars: 24000,
                max_web_search_results: 10,
                max_web_fetch_redirects: 10,
                web_fetch_redirect_response_body_cap_bytes: 65536,
                voice_enabled: false,
                voice_local_enabled: false,
                fast_prefix_detection: true,
                enable_network_probe_mock: true,
                enable_title_generation_skip: true,
                enable_suggestion_mode_skip: true,
                enable_filepath_extraction_mock: true,
            },
            performance: PerformanceConfig {
                max_concurrent_requests: 100,
                request_timeout_seconds: 30,
                streaming_timeout_seconds: 60,
            },
            security: SecurityConfig {
                enable_rate_limiting: true,
                rate_limit_requests_per_minute: 1000,
            },
            messaging: MessagingConfig {
                platform: "discord".to_string(),
                rate_limit: 1,
                rate_window: 1.0,
            },
            http: HttpConfig {
                read_timeout: 60.0,
                write_timeout: 15.0,
                connect_timeout: 10.0,
            },
            voice: VoiceConfig {
                note_enabled: true,
                whisper_device: "cpu".to_string(),
                whisper_model: "base".to_string(),
                hf_token: String::new(),
            },
            bot: BotConfig {
                telegram_bot_token: None,
                allowed_telegram_user_id: None,
                discord_bot_token: None,
                allowed_discord_channels: None,
                claude_workspace: "./agent_workspace".to_string(),
                allowed_dir: String::new(),
                claude_cli_bin: "claude".to_string(),
                max_message_log_entries_per_chat: None,
            },
            nim: NimSettings::default(),
        }
    }
}

impl Config {
    pub fn load(path: &str) -> Result<Self, ConfigError> {
        let path = PathBuf::from(path);

        if !path.exists() {
            return Err(ConfigError::FileNotFound(path));
        }

        let content =
            std::fs::read_to_string(&path).map_err(|e| ConfigError::ParseError(e.to_string()))?;

        let mut config: Config =
            toml::from_str(&content).map_err(|e| ConfigError::ParseError(e.to_string()))?;

        config.load_env_vars();

        Ok(config)
    }

    fn load_env_vars(&mut self) {
        if let Ok(addr) = env::var("PXCLAUDE_ADDR") {
            self.server.addr = addr;
        }

        if let Ok(level) = env::var("PXCLAUDE_LOG_LEVEL") {
            self.logging.level = level;
        }

        if let Ok(log_file) = env::var("PXCLAUDE_LOG_FILE") {
            self.logging.file = Some(log_file);
        }

        if let Ok(enabled) = env::var("ENABLE_WEB_SERVER_TOOLS") {
            self.features.web_tools_enabled = enabled.parse().unwrap_or(false);
        }

        if let Ok(schemes) = env::var("WEB_FETCH_ALLOWED_SCHEMES") {
            self.features.web_fetch_egress_allowed_schemes = schemes
                .split(',')
                .map(|s| s.trim().to_lowercase())
                .filter(|s| !s.is_empty())
                .collect();
        }

        if let Ok(allow_private) = env::var("WEB_FETCH_EGRESS_ALLOW_PRIVATE_NETWORK") {
            self.features.web_fetch_egress_allow_private = allow_private.parse().unwrap_or(false);
        }

        if let Ok(allow_private) = env::var("WEB_FETCH_ALLOW_PRIVATE_NETWORKS") {
            self.features.web_fetch_egress_allow_private = allow_private.parse().unwrap_or(false);
        }

        if let Ok(max_chars) = env::var("MAX_WEB_FETCH_CHARS") {
            self.features.max_web_fetch_chars = max_chars.parse().unwrap_or(24000);
        }

        if let Ok(provider) = env::var("DEFAULT_PROVIDER") {
            self.providers.default_provider = provider;
        }

        if let Ok(api_key) = env::var("OPENAI_API_KEY") {
            self.providers.openai.api_key = Some(api_key);
        }

        if let Ok(api_key) = env::var("ANTHROPIC_API_KEY") {
            self.providers.anthropic.api_key = Some(api_key);
        }

        if let Ok(base_url) = env::var("ANTHROPIC_BASE_URL") {
            self.providers.anthropic.base_url = Some(base_url);
        }

        if let Ok(api_key) = env::var("OPEN_ROUTER_API_KEY") {
            self.providers.open_router.api_key = Some(api_key);
        }

        if let Ok(api_key) = env::var("DEEPSEEK_API_KEY") {
            self.providers.deepseek.api_key = Some(api_key);
        }

        if let Ok(base_url) = env::var("DEEPSEEK_BASE_URL") {
            self.providers.deepseek.base_url = Some(base_url);
        }

        if let Ok(api_key) = env::var("KIMI_API_KEY") {
            self.providers.kimi.api_key = Some(api_key);
        }

        if let Ok(base_url) = env::var("KIMI_BASE_URL") {
            self.providers.kimi.base_url = Some(base_url);
        }

        if let Ok(api_key) = env::var("SILICONFLOW_API_KEY") {
            self.providers.siliconflow.api_key = Some(api_key);
        }

        if let Ok(api_key) = env::var("FIREWORKS_API_KEY") {
            self.providers.fireworks.api_key = Some(api_key);
        }

        if let Ok(api_key) = env::var("ZAI_API_KEY") {
            self.providers.z_ai.api_key = Some(api_key);
        }

        if let Ok(api_key) = env::var("GEMINI_API_KEY") {
            self.providers.gemini.api_key = Some(api_key);
        }

        if let Ok(base_url) = env::var("OPEN_ROUTER_BASE_URL") {
            self.providers.open_router.base_url = base_url;
        }

        if let Ok(referer) = env::var("OPEN_ROUTER_REFERER") {
            self.providers.open_router.referer = Some(referer);
        }

        if let Ok(base_url) = env::var("LLAMACPP_BASE_URL") {
            self.providers.llamacpp.base_url = Some(base_url);
        }

        if let Ok(base_url) = env::var("LM_STUDIO_BASE_URL") {
            self.providers.lmstudio.base_url = Some(base_url);
        }

        if let Ok(base_url) = env::var("OLLAMA_BASE_URL") {
            self.providers.ollama.base_url = Some(base_url);
        }

        if let Ok(base_url) = env::var("NVIDIA_NIM_BASE_URL") {
            self.providers.nvidia_nim.base_url = Some(base_url);
        }

        if let Ok(api_key) = env::var("NVIDIA_NIM_API_KEY") {
            self.providers.nvidia_nim.api_key = Some(api_key);
        }

        if let Ok(model_id) = env::var("NVIDIA_NIM_MODEL_ID") {
            self.providers.nvidia_nim.model_id = Some(model_id);
        }

        if let Ok(base_url) = env::var("FIREWORKS_BASE_URL") {
            self.providers.fireworks.base_url = Some(base_url);
        }

        if let Ok(base_url) = env::var("SILICONFLOW_BASE_URL") {
            self.providers.siliconflow.base_url = Some(base_url);
        }

        if let Ok(base_url) = env::var("ZAI_BASE_URL") {
            self.providers.z_ai.base_url = Some(base_url);
        }

        if let Ok(base_url) = env::var("GEMINI_BASE_URL") {
            self.providers.gemini.base_url = Some(base_url);
        }

        if let Ok(api_key) = env::var("CLOUDFLARE_GATEWAY_API_KEY") {
            self.providers.cloudflare_gateway.api_key = Some(api_key);
        }

        if let Ok(base_url) = env::var("CLOUDFLARE_GATEWAY_BASE_URL") {
            self.providers.cloudflare_gateway.base_url = Some(base_url);
        }

        if let Ok(model) = env::var("MODEL") {
            self.providers.model = model;
        }

        if let Ok(model_opus) = env::var("MODEL_OPUS") {
            if !model_opus.is_empty() {
                self.providers.model_opus = Some(model_opus);
            }
        }

        if let Ok(model_sonnet) = env::var("MODEL_SONNET") {
            if !model_sonnet.is_empty() {
                self.providers.model_sonnet = Some(model_sonnet);
            }
        }

        if let Ok(model_haiku) = env::var("MODEL_HAIKU") {
            if !model_haiku.is_empty() {
                self.providers.model_haiku = Some(model_haiku);
            }
        }

        if let Ok(enabled) = env::var("ENABLE_MODEL_THINKING") {
            self.providers.enable_model_thinking = enabled.parse().unwrap_or(true);
        }

        if let Ok(raw_payloads) = env::var("LOG_RAW_API_PAYLOADS") {
            self.logging.raw_api_payloads = raw_payloads.parse().unwrap_or(false);
        }

        if let Ok(raw_sse) = env::var("LOG_RAW_SSE_EVENTS") {
            self.logging.raw_sse_events = raw_sse.parse().unwrap_or(false);
        }

        if let Ok(raw_cli) = env::var("LOG_RAW_CLI_DIAGNOSTICS") {
            self.logging.raw_cli_diagnostics = raw_cli.parse().unwrap_or(false);
        }

        if let Ok(raw_messaging) = env::var("LOG_RAW_MESSAGING_CONTENT") {
            self.logging.raw_messaging_content = raw_messaging.parse().unwrap_or(false);
        }

        if let Ok(err_details) = env::var("LOG_MESSAGING_ERROR_DETAILS") {
            self.logging.messaging_error_details = err_details.parse().unwrap_or(false);
        }

        if let Ok(tracebacks) = env::var("LOG_API_ERROR_TRACEBACKS") {
            self.logging.error_tracebacks = tracebacks.parse().unwrap_or(false);
        }

        if let Ok(debug_edits) = env::var("DEBUG_PLATFORM_EDITS") {
            self.logging.debug_platform_edits = debug_edits.parse().unwrap_or(false);
        }

        if let Ok(debug_stack) = env::var("DEBUG_SUBAGENT_STACK") {
            self.logging.debug_subagent_stack = debug_stack.parse().unwrap_or(false);
        }

        if let Ok(auth_token) = env::var("ANTHROPIC_AUTH_TOKEN") {
            self.server.anthropic_auth_token = auth_token;
        }

        if let Ok(messaging_platform) = env::var("MESSAGING_PLATFORM") {
            self.messaging.platform = messaging_platform;
        }

        if let Ok(rate_limit) = env::var("MESSAGING_RATE_LIMIT") {
            if let Ok(val) = rate_limit.parse() {
                self.messaging.rate_limit = val;
            }
        }

        if let Ok(rate_window) = env::var("MESSAGING_RATE_WINDOW") {
            if let Ok(val) = rate_window.parse() {
                self.messaging.rate_window = val;
            }
        }

        if let Ok(rate_limit) = env::var("PROVIDER_RATE_LIMIT") {
            if let Ok(val) = rate_limit.parse() {
                self.providers.rate_limit = val;
            }
        }

        if let Ok(rate_window) = env::var("PROVIDER_RATE_WINDOW") {
            if let Ok(val) = rate_window.parse() {
                self.providers.rate_window = val;
            }
        }

        if let Ok(max_concurrency) = env::var("PROVIDER_MAX_CONCURRENCY") {
            if let Ok(val) = max_concurrency.parse() {
                self.providers.max_concurrency = val;
            }
        }

        if let Ok(read_timeout) = env::var("HTTP_READ_TIMEOUT") {
            if let Ok(val) = read_timeout.parse() {
                self.http.read_timeout = val;
            }
        }

        if let Ok(write_timeout) = env::var("HTTP_WRITE_TIMEOUT") {
            if let Ok(val) = write_timeout.parse() {
                self.http.write_timeout = val;
            }
        }

        if let Ok(connect_timeout) = env::var("HTTP_CONNECT_TIMEOUT") {
            if let Ok(val) = connect_timeout.parse() {
                self.http.connect_timeout = val;
            }
        }

        if let Ok(voice_enabled) = env::var("VOICE_NOTE_ENABLED") {
            self.voice.note_enabled = voice_enabled.parse().unwrap_or(true);
        }

        if let Ok(whisper_device) = env::var("WHISPER_DEVICE") {
            self.voice.whisper_device = whisper_device;
        }

        if let Ok(whisper_model) = env::var("WHISPER_MODEL") {
            self.voice.whisper_model = whisper_model;
        }

        if let Ok(hf_token) = env::var("HF_TOKEN") {
            self.voice.hf_token = hf_token;
        }

        if let Ok(token) = env::var("TELEGRAM_BOT_TOKEN") {
            self.bot.telegram_bot_token = Some(token);
        }

        if let Ok(user_id) = env::var("ALLOWED_TELEGRAM_USER_ID") {
            self.bot.allowed_telegram_user_id = Some(user_id);
        }

        if let Ok(token) = env::var("DISCORD_BOT_TOKEN") {
            self.bot.discord_bot_token = Some(token);
        }

        if let Ok(channels) = env::var("ALLOWED_DISCORD_CHANNELS") {
            self.bot.allowed_discord_channels = Some(channels);
        }

        if let Ok(workspace) = env::var("CLAUDE_WORKSPACE") {
            self.bot.claude_workspace = workspace;
        }

        if let Ok(allowed_dir) = env::var("ALLOWED_DIR") {
            self.bot.allowed_dir = allowed_dir;
        }

        if let Ok(cli_bin) = env::var("CLAUDE_CLI_BIN") {
            self.bot.claude_cli_bin = cli_bin;
        }

        if let Ok(max_entries) = env::var("MAX_MESSAGE_LOG_ENTRIES_PER_CHAT") {
            if !max_entries.is_empty() {
                if let Ok(val) = max_entries.parse() {
                    self.bot.max_message_log_entries_per_chat = Some(val);
                }
            }
        }

        // Per-provider proxies
        if let Ok(proxy) = env::var("NVIDIA_NIM_PROXY") {
            self.providers.nvidia_nim.proxy = Some(proxy);
        }
        if let Ok(proxy) = env::var("OPENROUTER_PROXY") {
            self.providers.open_router.proxy = Some(proxy);
        }
        if let Ok(proxy) = env::var("LMSTUDIO_PROXY") {
            self.providers.lmstudio.proxy = Some(proxy);
        }
        if let Ok(proxy) = env::var("LLAMACPP_PROXY") {
            self.providers.llamacpp.proxy = Some(proxy);
        }
        if let Ok(proxy) = env::var("KIMI_PROXY") {
            self.providers.kimi.proxy = Some(proxy);
        }
        if let Ok(proxy) = env::var("ZAI_PROXY") {
            self.providers.z_ai.proxy = Some(proxy);
        }
        if let Ok(proxy) = env::var("CF_GATEWAY_PROXY") {
            self.providers.cloudflare_gateway.proxy = Some(proxy);
        }
        if let Ok(proxy) = env::var("GEMINI_PROXY") {
            self.providers.gemini.proxy = Some(proxy);
        }
        if let Ok(proxy) = env::var("OPENAI_PROXY") {
            self.providers.openai.proxy = Some(proxy);
        }
        if let Ok(proxy) = env::var("ANTHROPIC_PROXY") {
            self.providers.anthropic.proxy = Some(proxy);
        }
        if let Ok(proxy) = env::var("SILICONFLOW_PROXY") {
            self.providers.siliconflow.proxy = Some(proxy);
        }
        if let Ok(proxy) = env::var("FIREWORKS_PROXY") {
            self.providers.fireworks.proxy = Some(proxy);
        }

        // CF gateway specific aliases
        if let Ok(base_url) = env::var("CF_GATEWAY_BASE_URL") {
            self.providers.cloudflare_gateway.base_url = Some(base_url);
        }
        if let Ok(api_key) = env::var("CF_AIG_TOKEN") {
            self.providers.cloudflare_gateway.api_key = Some(api_key);
        }
    }

    pub fn to_toml_string(&self) -> Result<String, ConfigError> {
        toml::to_string_pretty(self).map_err(|e| ConfigError::ParseError(e.to_string()))
    }

    pub fn get_provider_api_key(&self, provider: &str) -> Option<String> {
        match provider {
            "openai" => self.providers.openai.api_key.clone(),
            "anthropic" => self.providers.anthropic.api_key.clone(),
            "open_router" => self.providers.open_router.api_key.clone(),
            "deepseek" => self.providers.deepseek.api_key.clone(),
            "kimi" => self.providers.kimi.api_key.clone(),
            "siliconflow" => self.providers.siliconflow.api_key.clone(),
            "fireworks" => self.providers.fireworks.api_key.clone(),
            "z_ai" => self.providers.z_ai.api_key.clone(),
            "gemini" => self.providers.gemini.api_key.clone(),
            "nvidia_nim" => self.providers.nvidia_nim.api_key.clone(),
            "ollama" => self.providers.ollama.api_key.clone(),
            "cloudflare_gateway" => self.providers.cloudflare_gateway.api_key.clone(),
            _ => None,
        }
    }

    pub fn get_provider_base_url(&self, provider: &str) -> Option<String> {
        match provider {
            "openai" => self.providers.openai.base_url.clone(),
            "open_router" => Some(self.providers.open_router.base_url.clone()),
            "nvidia_nim" => self.providers.nvidia_nim.base_url.clone(),
            "llamacpp" => self.providers.llamacpp.base_url.clone(),
            "lmstudio" => self.providers.lmstudio.base_url.clone(),
            "ollama" => self.providers.ollama.base_url.clone(),
            "fireworks" => self.providers.fireworks.base_url.clone(),
            "siliconflow" => self.providers.siliconflow.base_url.clone(),
            "z_ai" => self.providers.z_ai.base_url.clone(),
            "gemini" => self.providers.gemini.base_url.clone(),
            "anthropic" => self.providers.anthropic.base_url.clone(),
            "deepseek" => self.providers.deepseek.base_url.clone(),
            "kimi" => self.providers.kimi.base_url.clone(),
            "cloudflare_gateway" => self.providers.cloudflare_gateway.base_url.clone(),
            _ => None,
        }
    }

    pub fn get_provider_proxy(&self, provider: &str) -> Option<String> {
        match provider {
            "openai" => self.providers.openai.proxy.clone(),
            "anthropic" => self.providers.anthropic.proxy.clone(),
            "open_router" => self.providers.open_router.proxy.clone(),
            "nvidia_nim" => self.providers.nvidia_nim.proxy.clone(),
            "deepseek" => self.providers.deepseek.proxy.clone(),
            "kimi" => self.providers.kimi.proxy.clone(),
            "llamacpp" => self.providers.llamacpp.proxy.clone(),
            "lmstudio" => self.providers.lmstudio.proxy.clone(),
            "fireworks" => self.providers.fireworks.proxy.clone(),
            "siliconflow" => self.providers.siliconflow.proxy.clone(),
            "z_ai" => self.providers.z_ai.proxy.clone(),
            "gemini" => self.providers.gemini.proxy.clone(),
            "ollama" => self.providers.ollama.proxy.clone(),
            "cloudflare_gateway" => self.providers.cloudflare_gateway.proxy.clone(),
            _ => None,
        }
    }
}
