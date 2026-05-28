use crate::config::Config;
use crate::models::{MessagesRequest, TokenCountRequest};

const GATEWAY_MODEL_ID_PREFIX: &str = "anthropic";
const NO_THINKING_GATEWAY_MODEL_ID_PREFIX: &str = "claude-3-proxycc-no-thinking";

const SUPPORTED_PROVIDER_IDS: &[&str] = &[
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

#[derive(Debug, Clone)]
pub struct ResolvedModel {
    pub original_model: String,
    pub provider_id: String,
    pub provider_model: String,
    pub provider_model_ref: String,
    pub thinking_enabled: bool,
}

#[derive(Debug, Clone)]
pub struct RoutedMessagesRequest {
    pub request: MessagesRequest,
    pub resolved: ResolvedModel,
}

#[derive(Debug, Clone)]
pub struct RoutedTokenCountRequest {
    pub request: TokenCountRequest,
    pub resolved: ResolvedModel,
}

#[derive(Debug, Clone)]
pub struct DecodedGatewayModelId {
    pub provider_id: String,
    pub provider_model: String,
    pub force_thinking_enabled: Option<bool>,
}

pub fn gateway_model_id(provider_model_ref: &str) -> String {
    format!("{GATEWAY_MODEL_ID_PREFIX}/{provider_model_ref}")
}

pub fn no_thinking_gateway_model_id(provider_model_ref: &str) -> String {
    format!("{NO_THINKING_GATEWAY_MODEL_ID_PREFIX}/{provider_model_ref}")
}

pub fn decode_gateway_model_id(model_name: &str) -> Option<DecodedGatewayModelId> {
    let (prefix, remainder) = model_name.split_once('/')?;

    let force_thinking_enabled = if prefix == GATEWAY_MODEL_ID_PREFIX {
        None
    } else if prefix == NO_THINKING_GATEWAY_MODEL_ID_PREFIX {
        Some(false)
    } else {
        return None;
    };

    let (provider_id, provider_model) = remainder.split_once('/')?;
    if provider_model.is_empty() {
        return None;
    }

    Some(DecodedGatewayModelId {
        provider_id: provider_id.to_string(),
        provider_model: provider_model.to_string(),
        force_thinking_enabled,
    })
}

pub struct ModelRouter {
    config: Config,
}

impl ModelRouter {
    pub fn new(config: Config) -> Self {
        Self { config }
    }

    pub fn resolve(&self, claude_model_name: &str) -> ResolvedModel {
        if let Some(decoded) = self.direct_provider_model(claude_model_name) {
            return decoded;
        }

        let provider_model_ref = self.resolve_model(claude_model_name).to_string();
        let thinking_enabled = self.resolve_thinking(claude_model_name);

        let provider_model = Self::parse_model_name(&provider_model_ref);

        ResolvedModel {
            original_model: claude_model_name.to_string(),
            provider_id: Self::parse_provider_type(&provider_model_ref),
            provider_model,
            provider_model_ref,
            thinking_enabled,
        }
    }

    fn resolve_model(&self, claude_model_name: &str) -> &str {
        let name_lower = claude_model_name.to_lowercase();
        if name_lower.contains("opus") {
            if let Some(ref m) = self.config.providers.model_opus {
                return m;
            }
        }
        if name_lower.contains("haiku") {
            if let Some(ref m) = self.config.providers.model_haiku {
                return m;
            }
        }
        if name_lower.contains("sonnet") {
            if let Some(ref m) = self.config.providers.model_sonnet {
                return m;
            }
        }
        &self.config.providers.model
    }

    fn resolve_thinking(&self, claude_model_name: &str) -> bool {
        let name_lower = claude_model_name.to_lowercase();
        if name_lower.contains("opus") {
            if let Some(enabled) = self.config.providers.enable_opus_thinking {
                return enabled;
            }
        }
        if name_lower.contains("haiku") {
            if let Some(enabled) = self.config.providers.enable_haiku_thinking {
                return enabled;
            }
        }
        if name_lower.contains("sonnet") {
            if let Some(enabled) = self.config.providers.enable_sonnet_thinking {
                return enabled;
            }
        }
        self.config.providers.enable_model_thinking
    }

    fn direct_provider_model(&self, claude_model_name: &str) -> Option<ResolvedModel> {
        let (provider_id, provider_model) = {
            let decoded = decode_gateway_model_id(claude_model_name);
            match decoded {
                Some(d) => {
                    if !SUPPORTED_PROVIDER_IDS.contains(&d.provider_id.as_str()) {
                        return None;
                    }
                    (d.provider_id, d.provider_model)
                }
                None => {
                    let (provider_id, separator, provider_model) = match claude_model_name.split_once('/') {
                        Some((p, m)) => (p.to_string(), true, m.to_string()),
                        None => return None,
                    };
                    if !separator || provider_model.is_empty() {
                        return None;
                    }
                    if !SUPPORTED_PROVIDER_IDS.contains(&provider_id.as_str()) {
                        return None;
                    }
                    (provider_id, provider_model)
                }
            }
        };

        Some(ResolvedModel {
            original_model: claude_model_name.to_string(),
            provider_id: provider_id.clone(),
            provider_model: provider_model.clone(),
            provider_model_ref: claude_model_name.to_string(),
            thinking_enabled: self.resolve_thinking(claude_model_name),
        })
    }

    pub fn resolve_messages_request(
        &self,
        request: &MessagesRequest,
    ) -> RoutedMessagesRequest {
        let resolved = self.resolve(&request.model);
        let mut routed = request.clone();
        routed.model = resolved.provider_model.clone();
        RoutedMessagesRequest {
            request: routed,
            resolved,
        }
    }

    pub fn resolve_token_count_request(
        &self,
        request: &TokenCountRequest,
    ) -> RoutedTokenCountRequest {
        let resolved = self.resolve(&request.model);
        let mut routed = request.clone();
        routed.model = resolved.provider_model.clone();
        RoutedTokenCountRequest {
            request: routed,
            resolved,
        }
    }

    pub fn parse_provider_type(model_string: &str) -> String {
        model_string
            .split('/')
            .next()
            .unwrap_or("")
            .to_string()
    }

    pub fn parse_model_name(model_string: &str) -> String {
        model_string
            .splitn(2, '/')
            .nth(1)
            .unwrap_or("")
            .to_string()
    }
}
