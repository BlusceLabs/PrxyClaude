#[derive(Debug, Clone, PartialEq)]
pub enum TransportType {
    OpenAIChat,
    AnthropicMessages,
}

impl TransportType {
    pub fn as_str(&self) -> &'static str {
        match self {
            TransportType::OpenAIChat => "openai_chat",
            TransportType::AnthropicMessages => "anthropic_messages",
        }
    }
}

pub struct ProviderDescriptor {
    pub provider_id: &'static str,
    pub transport_type: TransportType,
    pub credential_env: Option<&'static str>,
    pub credential_url: Option<&'static str>,
    pub credential_attr: Option<&'static str>,
    pub static_credential: Option<&'static str>,
    pub default_base_url: Option<&'static str>,
    pub base_url_attr: Option<&'static str>,
    pub proxy_attr: Option<&'static str>,
    pub capabilities: &'static [&'static str],
}

pub const NVIDIA_NIM_DEFAULT_BASE: &str = "https://integrate.api.nvidia.com/v1";
pub const KIMI_DEFAULT_BASE: &str = "https://api.moonshot.ai/v1";
pub const DEEPSEEK_ANTHROPIC_DEFAULT_BASE: &str = "https://api.deepseek.com/anthropic";
pub const DEEPSEEK_DEFAULT_BASE: &str = "https://api.deepseek.com/anthropic";
pub const OPENROUTER_DEFAULT_BASE: &str = "https://openrouter.ai/api/v1";
pub const LMSTUDIO_DEFAULT_BASE: &str = "http://localhost:1234/v1";
pub const LLAMACPP_DEFAULT_BASE: &str = "http://localhost:8080/v1";
pub const OLLAMA_DEFAULT_BASE: &str = "http://localhost:11434";
pub const ZAI_DEFAULT_BASE: &str = "https://api.z.ai/api/paas/v4";
pub const GEMINI_DEFAULT_BASE: &str = "https://generativelanguage.googleapis.com/v1beta/openai";
pub const CF_GATEWAY_V1_DEFAULT_BASE: &str =
    "https://gateway.ai.cloudflare.com/v1/ACCOUNT_ID/GATEWAY_NAME/anthropic/v1";
pub const OPENAI_DEFAULT_BASE: &str = "https://api.openai.com/v1";
pub const ANTHROPIC_DEFAULT_BASE: &str = "https://api.anthropic.com/v1";
pub const SILICONFLOW_DEFAULT_BASE: &str = "https://api.siliconflow.com/v1";
pub const FIREWORKS_DEFAULT_BASE: &str = "https://api.fireworks.ai/inference/v1";

pub const PROVIDER_CATALOG: &[ProviderDescriptor] = &[
    ProviderDescriptor {
        provider_id: "nvidia_nim",
        transport_type: TransportType::OpenAIChat,
        credential_env: Some("NVIDIA_NIM_API_KEY"),
        credential_url: Some("https://build.nvidia.com/settings/api-keys"),
        credential_attr: Some("nvidia_nim_api_key"),
        static_credential: None,
        default_base_url: Some(NVIDIA_NIM_DEFAULT_BASE),
        base_url_attr: None,
        proxy_attr: Some("nvidia_nim_proxy"),
        capabilities: &["chat", "streaming", "tools", "thinking", "rate_limit"],
    },
    ProviderDescriptor {
        provider_id: "open_router",
        transport_type: TransportType::AnthropicMessages,
        credential_env: Some("OPENROUTER_API_KEY"),
        credential_url: Some("https://openrouter.ai/keys"),
        credential_attr: Some("open_router_api_key"),
        static_credential: None,
        default_base_url: Some(OPENROUTER_DEFAULT_BASE),
        base_url_attr: None,
        proxy_attr: Some("open_router_proxy"),
        capabilities: &["chat", "streaming", "tools", "thinking", "native_anthropic"],
    },
    ProviderDescriptor {
        provider_id: "deepseek",
        transport_type: TransportType::AnthropicMessages,
        credential_env: Some("DEEPSEEK_API_KEY"),
        credential_url: Some("https://platform.deepseek.com/api_keys"),
        credential_attr: Some("deepseek_api_key"),
        static_credential: None,
        default_base_url: Some(DEEPSEEK_ANTHROPIC_DEFAULT_BASE),
        base_url_attr: None,
        proxy_attr: None,
        capabilities: &["chat", "streaming", "tools", "thinking", "native_anthropic"],
    },
    ProviderDescriptor {
        provider_id: "lmstudio",
        transport_type: TransportType::AnthropicMessages,
        credential_env: None,
        credential_url: None,
        credential_attr: None,
        static_credential: Some("lm-studio"),
        default_base_url: Some(LMSTUDIO_DEFAULT_BASE),
        base_url_attr: Some("lm_studio_base_url"),
        proxy_attr: Some("lmstudio_proxy"),
        capabilities: &["chat", "streaming", "tools", "native_anthropic", "local"],
    },
    ProviderDescriptor {
        provider_id: "llamacpp",
        transport_type: TransportType::AnthropicMessages,
        credential_env: None,
        credential_url: None,
        credential_attr: None,
        static_credential: Some("llamacpp"),
        default_base_url: Some(LLAMACPP_DEFAULT_BASE),
        base_url_attr: Some("llamacpp_base_url"),
        proxy_attr: Some("llamacpp_proxy"),
        capabilities: &["chat", "streaming", "tools", "native_anthropic", "local"],
    },
    ProviderDescriptor {
        provider_id: "ollama",
        transport_type: TransportType::AnthropicMessages,
        credential_env: None,
        credential_url: None,
        credential_attr: None,
        static_credential: Some("ollama"),
        default_base_url: Some(OLLAMA_DEFAULT_BASE),
        base_url_attr: None,
        proxy_attr: None,
        capabilities: &["chat", "streaming", "tools", "thinking", "native_anthropic", "local"],
    },
    ProviderDescriptor {
        provider_id: "kimi",
        transport_type: TransportType::OpenAIChat,
        credential_env: Some("KIMI_API_KEY"),
        credential_url: Some("https://platform.moonshot.cn/console/api-keys"),
        credential_attr: Some("kimi_api_key"),
        static_credential: None,
        default_base_url: Some(KIMI_DEFAULT_BASE),
        base_url_attr: None,
        proxy_attr: Some("kimi_proxy"),
        capabilities: &["chat", "streaming", "tools"],
    },
    ProviderDescriptor {
        provider_id: "z_ai",
        transport_type: TransportType::OpenAIChat,
        credential_env: Some("ZAI_API_KEY"),
        credential_url: Some("https://z.ai/manage-apikey/apikey-list"),
        credential_attr: Some("z_ai_api_key"),
        static_credential: None,
        default_base_url: Some(ZAI_DEFAULT_BASE),
        base_url_attr: None,
        proxy_attr: Some("z_ai_proxy"),
        capabilities: &["chat", "streaming", "tools"],
    },
    ProviderDescriptor {
        provider_id: "cloudflare_gateway",
        transport_type: TransportType::AnthropicMessages,
        credential_env: Some("CF_AIG_TOKEN"),
        credential_url: Some("https://dash.cloudflare.com/profile/api-tokens"),
        credential_attr: Some("cf_aig_token"),
        static_credential: None,
        default_base_url: Some(CF_GATEWAY_V1_DEFAULT_BASE),
        base_url_attr: Some("cf_gateway_base_url"),
        proxy_attr: Some("cf_gateway_proxy"),
        capabilities: &["chat", "streaming", "tools", "thinking", "native_anthropic"],
    },
    ProviderDescriptor {
        provider_id: "gemini",
        transport_type: TransportType::OpenAIChat,
        credential_env: Some("GEMINI_API_KEY"),
        credential_url: Some("https://aistudio.google.com/apikey"),
        credential_attr: Some("gemini_api_key"),
        static_credential: None,
        default_base_url: Some(GEMINI_DEFAULT_BASE),
        base_url_attr: None,
        proxy_attr: Some("gemini_proxy"),
        capabilities: &["chat", "streaming", "tools", "thinking"],
    },
    ProviderDescriptor {
        provider_id: "openai",
        transport_type: TransportType::OpenAIChat,
        credential_env: Some("OPENAI_API_KEY"),
        credential_url: Some("https://platform.openai.com/api-keys"),
        credential_attr: Some("openai_api_key"),
        static_credential: None,
        default_base_url: Some(OPENAI_DEFAULT_BASE),
        base_url_attr: None,
        proxy_attr: Some("openai_proxy"),
        capabilities: &["chat", "streaming", "tools", "thinking"],
    },
    ProviderDescriptor {
        provider_id: "anthropic",
        transport_type: TransportType::AnthropicMessages,
        credential_env: Some("ANTHROPIC_API_KEY"),
        credential_url: Some("https://console.anthropic.com/settings/keys"),
        credential_attr: Some("anthropic_api_key"),
        static_credential: None,
        default_base_url: Some(ANTHROPIC_DEFAULT_BASE),
        base_url_attr: None,
        proxy_attr: Some("anthropic_proxy"),
        capabilities: &["chat", "streaming", "tools", "thinking", "native_anthropic"],
    },
    ProviderDescriptor {
        provider_id: "siliconflow",
        transport_type: TransportType::OpenAIChat,
        credential_env: Some("SILICONFLOW_API_KEY"),
        credential_url: Some("https://cloud.siliconflow.cn/account/ak"),
        credential_attr: Some("siliconflow_api_key"),
        static_credential: None,
        default_base_url: Some(SILICONFLOW_DEFAULT_BASE),
        base_url_attr: None,
        proxy_attr: Some("siliconflow_proxy"),
        capabilities: &["chat", "streaming", "tools"],
    },
    ProviderDescriptor {
        provider_id: "fireworks",
        transport_type: TransportType::OpenAIChat,
        credential_env: Some("FIREWORKS_API_KEY"),
        credential_url: Some("https://app.fireworks.ai/users/keys"),
        credential_attr: Some("fireworks_api_key"),
        static_credential: None,
        default_base_url: Some(FIREWORKS_DEFAULT_BASE),
        base_url_attr: None,
        proxy_attr: Some("fireworks_proxy"),
        capabilities: &["chat", "streaming", "tools"],
    },
];

pub const SUPPORTED_PROVIDER_IDS: &[&str] = &[
    "nvidia_nim",
    "open_router",
    "deepseek",
    "lmstudio",
    "llamacpp",
    "ollama",
    "kimi",
    "z_ai",
    "cloudflare_gateway",
    "gemini",
    "openai",
    "anthropic",
    "siliconflow",
    "fireworks",
];
