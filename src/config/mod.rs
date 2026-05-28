use serde::{Deserialize, Serialize};

use std::env;
use std::path::PathBuf;
use thiserror::Error;

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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub addr: String,
    pub cors_enabled: bool,
    pub cors_origins: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    pub level: String,
    pub file: Option<String>,
    pub raw_api_payloads: bool,
    pub error_tracebacks: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub default_provider: String,
    pub openai: OpenAIConfig,
    pub anthropic: AnthropicConfig,
    pub open_router: OpenRouterConfig,
    pub nvidia_nim: NvidiaNimConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAIConfig {
    pub api_key: Option<String>,
    pub base_url: Option<String>,
    pub organization: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnthropicConfig {
    pub api_key: Option<String>,
    pub beta_features: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenRouterConfig {
    pub api_key: Option<String>,
    pub base_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NvidiaNimConfig {
    pub base_url: Option<String>,
    pub model_id: Option<String>,
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

impl Default for Config {
    fn default() -> Self {
        Self {
            server: ServerConfig {
                addr: "127.0.0.1:8080".to_string(),
                cors_enabled: true,
                cors_origins: vec![
                    "http://localhost:3000".to_string(),
                    "http://localhost:8080".to_string(),
                ],
            },
            logging: LoggingConfig {
                level: "info".to_string(),
                file: None,
                raw_api_payloads: false,
                error_tracebacks: false,
            },
            providers: ProviderConfig {
                default_provider: "open_router".to_string(),
                openai: OpenAIConfig {
                    api_key: None,
                    base_url: None,
                    organization: None,
                },
                anthropic: AnthropicConfig {
                    api_key: None,
                    beta_features: vec![],
                },
                open_router: OpenRouterConfig {
                    api_key: None,
                    base_url: "https://openrouter.ai/api/v1".to_string(),
                },
                nvidia_nim: NvidiaNimConfig {
                    base_url: None,
                    model_id: None,
                },
            },
            features: FeatureConfig {
                web_tools_enabled: true,
                web_fetch_egress_allow_private: false,
                web_fetch_egress_allowed_schemes: vec!["https".to_string()],
                max_web_fetch_chars: 24000,
                max_web_search_results: 10,
                max_web_fetch_redirects: 10,
                web_fetch_redirect_response_body_cap_bytes: 65536,
                voice_enabled: false,
                voice_local_enabled: false,
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
        }
    }
}

impl Config {
    pub fn load(path: &str) -> Result<Self, ConfigError> {
        let path = PathBuf::from(path);
        
        if !path.exists() {
            return Err(ConfigError::FileNotFound(path));
        }
        
        let content = std::fs::read_to_string(&path)
            .map_err(|e| ConfigError::ParseError(e.to_string()))?;
        
        let mut config: Config = toml::from_str(&content)
            .map_err(|e| ConfigError::ParseError(e.to_string()))?;
        
        // Override with environment variables
        config.load_env_vars();
        
        Ok(config)
    }
    
    fn load_env_vars(&mut self) {
        // Server configuration
        if let Ok(addr) = env::var("PXCLAUDE_ADDR") {
            self.server.addr = addr;
        }
        
        if let Ok(level) = env::var("PXCLAUDE_LOG_LEVEL") {
            self.logging.level = level;
        }
        
        if let Ok(log_file) = env::var("PXCLAUDE_LOG_FILE") {
            self.logging.file = Some(log_file);
        }
        
        // Feature configuration
        if let Ok(enabled) = env::var("ENABLE_WEB_SERVER_TOOLS") {
            self.features.web_tools_enabled = enabled.parse().unwrap_or(false);
        }
        
        if let Ok(allow_private) = env::var("WEB_FETCH_EGRESS_ALLOW_PRIVATE_NETWORK") {
            self.features.web_fetch_egress_allow_private = allow_private.parse().unwrap_or(false);
        }
        
        if let Ok(max_chars) = env::var("MAX_WEB_FETCH_CHARS") {
            self.features.max_web_fetch_chars = max_chars.parse().unwrap_or(24000);
        }
        
        // Provider configuration
        if let Ok(provider) = env::var("DEFAULT_PROVIDER") {
            self.providers.default_provider = provider;
        }
        
        if let Ok(api_key) = env::var("OPENAI_API_KEY") {
            self.providers.openai.api_key = Some(api_key);
        }
        
        if let Ok(api_key) = env::var("ANTHROPIC_API_KEY") {
            self.providers.anthropic.api_key = Some(api_key);
        }
        
        if let Ok(api_key) = env::var("OPEN_ROUTER_API_KEY") {
            self.providers.open_router.api_key = Some(api_key);
        }
        
        // Logging configuration
        if let Ok(tracebacks) = env::var("LOG_API_ERROR_TRACEBACKS") {
            self.logging.error_tracebacks = tracebacks.parse().unwrap_or(false);
        }
        
        if let Ok(raw_payloads) = env::var("LOG_RAW_API_PAYLOADS") {
            self.logging.raw_api_payloads = raw_payloads.parse().unwrap_or(false);
        }
    }
    
    pub fn to_toml_string(&self) -> Result<String, ConfigError> {
        toml::to_string_pretty(self)
            .map_err(|e| ConfigError::ParseError(e.to_string()))
    }
    
    pub fn get_provider_api_key(&self, provider: &str) -> Option<String> {
        match provider {
            "openai" => self.providers.openai.api_key.clone(),
            "anthropic" => self.providers.anthropic.api_key.clone(),
            "open_router" => self.providers.open_router.api_key.clone(),
            "nvidia_nim" => None, // NIM doesn't need API key
            _ => None,
        }
    }
    
    pub fn get_provider_base_url(&self, provider: &str) -> Option<String> {
        match provider {
            "openai" => self.providers.openai.base_url.clone(),
            "open_router" => Some(self.providers.open_router.base_url.clone()),
            "nvidia_nim" => self.providers.nvidia_nim.base_url.clone(),
            _ => None,
        }
    }
}