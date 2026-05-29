//! Provider descriptors, factory, and runtime registry.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use tracing::{info, warn};

use crate::config::provider_catalog::{
    ProviderDescriptor, PROVIDER_CATALOG, SUPPORTED_PROVIDER_IDS,
};
use super::model_listing::{ProviderModelInfo, model_infos_from_ids};
use super::traits::{Provider, ProviderError};
use super::*;

/// Environment-based configuration values that mirror the Python Settings class.
pub struct EnvConfig {
    pub nvidia_nim_api_key: Option<String>,
    pub nvidia_nim_proxy: Option<String>,
    pub open_router_api_key: Option<String>,
    pub open_router_proxy: Option<String>,
    pub deepseek_api_key: Option<String>,
    pub lm_studio_base_url: Option<String>,
    pub lmstudio_proxy: Option<String>,
    pub llamacpp_base_url: Option<String>,
    pub llamacpp_proxy: Option<String>,
    pub ollama_base_url: Option<String>,
    pub kimi_api_key: Option<String>,
    pub kimi_proxy: Option<String>,
    pub z_ai_api_key: Option<String>,
    pub z_ai_proxy: Option<String>,
    pub cf_aig_token: Option<String>,
    pub cf_gateway_base_url: Option<String>,
    pub cf_gateway_proxy: Option<String>,
    pub gemini_api_key: Option<String>,
    pub gemini_proxy: Option<String>,
    pub openai_api_key: Option<String>,
    pub openai_proxy: Option<String>,
    pub anthropic_api_key: Option<String>,
    pub anthropic_proxy: Option<String>,
    pub siliconflow_api_key: Option<String>,
    pub siliconflow_proxy: Option<String>,
    pub fireworks_api_key: Option<String>,
    pub fireworks_proxy: Option<String>,
}

impl EnvConfig {
    /// Load configuration from environment variables.
    pub fn from_env() -> Self {
        Self {
            nvidia_nim_api_key: std::env::var("NVIDIA_NIM_API_KEY").ok(),
            nvidia_nim_proxy: std::env::var("NVIDIA_NIM_PROXY").ok(),
            open_router_api_key: std::env::var("OPENROUTER_API_KEY").ok(),
            open_router_proxy: std::env::var("OPENROUTER_PROXY").ok(),
            deepseek_api_key: std::env::var("DEEPSEEK_API_KEY").ok(),
            lm_studio_base_url: std::env::var("LM_STUDIO_BASE_URL").ok(),
            lmstudio_proxy: std::env::var("LMSTUDIO_PROXY").ok(),
            llamacpp_base_url: std::env::var("LLAMACPP_BASE_URL").ok(),
            llamacpp_proxy: std::env::var("LLAMACPP_PROXY").ok(),
            ollama_base_url: std::env::var("OLLAMA_BASE_URL").ok(),
            kimi_api_key: std::env::var("KIMI_API_KEY").ok(),
            kimi_proxy: std::env::var("KIMI_PROXY").ok(),
            z_ai_api_key: std::env::var("ZAI_API_KEY").ok(),
            z_ai_proxy: std::env::var("ZAI_PROXY").ok(),
            cf_aig_token: std::env::var("CF_AIG_TOKEN").ok(),
            cf_gateway_base_url: std::env::var("CF_GATEWAY_BASE_URL").ok(),
            cf_gateway_proxy: std::env::var("CF_GATEWAY_PROXY").ok(),
            gemini_api_key: std::env::var("GEMINI_API_KEY").ok(),
            gemini_proxy: std::env::var("GEMINI_PROXY").ok(),
            openai_api_key: std::env::var("OPENAI_API_KEY").ok(),
            openai_proxy: std::env::var("OPENAI_PROXY").ok(),
            anthropic_api_key: std::env::var("ANTHROPIC_API_KEY").ok(),
            anthropic_proxy: std::env::var("ANTHROPIC_PROXY").ok(),
            siliconflow_api_key: std::env::var("SILICONFLOW_API_KEY").ok(),
            siliconflow_proxy: std::env::var("SILICONFLOW_PROXY").ok(),
            fireworks_api_key: std::env::var("FIREWORKS_API_KEY").ok(),
            fireworks_proxy: std::env::var("FIREWORKS_PROXY").ok(),
        }
    }

    /// Look up a string attribute by name from this config.
    fn string_attr(&self, attr_name: Option<&str>, default: &str) -> String {
        match attr_name {
            None => default.to_string(),
            Some(name) => match name {
                "nvidia_nim_proxy" => self.nvidia_nim_proxy.clone().unwrap_or_default(),
                "open_router_proxy" => self.open_router_proxy.clone().unwrap_or_default(),
                "lm_studio_base_url" => self.lm_studio_base_url.clone().unwrap_or_default(),
                "lmstudio_proxy" => self.lmstudio_proxy.clone().unwrap_or_default(),
                "llamacpp_base_url" => self.llamacpp_base_url.clone().unwrap_or_default(),
                "llamacpp_proxy" => self.llamacpp_proxy.clone().unwrap_or_default(),
                "ollama_base_url" => self.ollama_base_url.clone().unwrap_or_default(),
                "kimi_proxy" => self.kimi_proxy.clone().unwrap_or_default(),
                "z_ai_proxy" => self.z_ai_proxy.clone().unwrap_or_default(),
                "cf_gateway_base_url" => self.cf_gateway_base_url.clone().unwrap_or_default(),
                "cf_gateway_proxy" => self.cf_gateway_proxy.clone().unwrap_or_default(),
                "gemini_proxy" => self.gemini_proxy.clone().unwrap_or_default(),
                "openai_proxy" => self.openai_proxy.clone().unwrap_or_default(),
                "anthropic_proxy" => self.anthropic_proxy.clone().unwrap_or_default(),
                "siliconflow_proxy" => self.siliconflow_proxy.clone().unwrap_or_default(),
                "fireworks_proxy" => self.fireworks_proxy.clone().unwrap_or_default(),
                _ => default.to_string(),
            },
        }
    }

    /// Look up a credential attribute by name from this config.
    fn credential_attr(&self, attr_name: Option<&str>) -> String {
        match attr_name {
            None => String::new(),
            Some(name) => match name {
                "nvidia_nim_api_key" => self.nvidia_nim_api_key.clone().unwrap_or_default(),
                "open_router_api_key" => self.open_router_api_key.clone().unwrap_or_default(),
                "deepseek_api_key" => self.deepseek_api_key.clone().unwrap_or_default(),
                "kimi_api_key" => self.kimi_api_key.clone().unwrap_or_default(),
                "z_ai_api_key" => self.z_ai_api_key.clone().unwrap_or_default(),
                "cf_aig_token" => self.cf_aig_token.clone().unwrap_or_default(),
                "gemini_api_key" => self.gemini_api_key.clone().unwrap_or_default(),
                "openai_api_key" => self.openai_api_key.clone().unwrap_or_default(),
                "anthropic_api_key" => self.anthropic_api_key.clone().unwrap_or_default(),
                "siliconflow_api_key" => self.siliconflow_api_key.clone().unwrap_or_default(),
                "fireworks_api_key" => self.fireworks_api_key.clone().unwrap_or_default(),
                _ => String::new(),
            },
        }
    }
}

/// Configuration for building a provider instance (distinct from config::ProviderConfig).
#[derive(Debug, Clone)]
pub struct ProviderBuildConfig {
    pub api_key: String,
    pub base_url: Option<String>,
    pub proxy: Option<String>,
}

/// Extract the credential for a provider descriptor from the env config.
fn credential_for(descriptor: &ProviderDescriptor, config: &EnvConfig) -> String {
    if let Some(static_cred) = descriptor.static_credential {
        return static_cred.to_string();
    }
    config.credential_attr(descriptor.credential_attr)
}

/// Validate that a credential is present if required by the descriptor.
fn require_credential(
    descriptor: &ProviderDescriptor,
    credential: &str,
) -> Result<(), ProviderError> {
    if descriptor.credential_env.is_none() {
        return Ok(());
    }
    if !credential.trim().is_empty() {
        return Ok(());
    }
    let mut message = format!(
        "{} is not set. Add it to your .env file.",
        descriptor.credential_env.unwrap_or("UNKNOWN")
    );
    if let Some(url) = descriptor.credential_url {
        message = format!("{}. Get a key at {}", message, url);
    }
    Err(ProviderError::authentication(&message))
}

/// Build a ProviderBuildConfig from a descriptor and env config.
pub fn build_provider_config(
    descriptor: &ProviderDescriptor,
    config: &EnvConfig,
) -> Result<ProviderBuildConfig, ProviderError> {
    let credential = credential_for(descriptor, config);
    require_credential(descriptor, &credential)?;

    let base_url = {
        let explicit = config.string_attr(descriptor.base_url_attr, "");
        if !explicit.is_empty() {
            Some(explicit)
        } else {
            descriptor.default_base_url.map(|s| s.to_string())
        }
    };

    let proxy = config.string_attr(descriptor.proxy_attr, "");

    Ok(ProviderBuildConfig {
        api_key: credential,
        base_url,
        proxy: if proxy.is_empty() { None } else { Some(proxy) },
    })
}

type ProviderFactory = Box<dyn Fn(&ProviderBuildConfig) -> Arc<dyn Provider> + Send + Sync>;

fn build_factory_map() -> HashMap<&'static str, ProviderFactory> {
    let mut m: HashMap<&'static str, ProviderFactory> = HashMap::new();

    m.insert(
        "nvidia_nim",
        Box::new(|cfg: &ProviderBuildConfig| {
            Arc::new(NvidiaNimProvider::new(
                cfg.base_url.clone(),
                None,
                Some(cfg.api_key.clone()),
            )) as Arc<dyn Provider>
        }),
    );
    m.insert(
        "open_router",
        Box::new(|cfg: &ProviderBuildConfig| {
            Arc::new(OpenRouterProvider::new(
                Some(cfg.api_key.clone()),
                cfg.base_url
                    .clone()
                    .unwrap_or_else(|| "https://openrouter.ai/api/v1".to_string()),
                cfg.proxy.clone(),
            )) as Arc<dyn Provider>
        }),
    );
    m.insert(
        "deepseek",
        Box::new(|cfg: &ProviderBuildConfig| {
            Arc::new(DeepSeekProvider::new(
                Some(cfg.api_key.clone()),
                cfg.base_url.clone(),
            )) as Arc<dyn Provider>
        }),
    );
    m.insert(
        "lmstudio",
        Box::new(|cfg: &ProviderBuildConfig| {
            Arc::new(LmstudioProvider::new(cfg.base_url.clone())) as Arc<dyn Provider>
        }),
    );
    m.insert(
        "llamacpp",
        Box::new(|cfg: &ProviderBuildConfig| {
            Arc::new(LlamaCppProvider::new(cfg.base_url.clone())) as Arc<dyn Provider>
        }),
    );
    m.insert(
        "ollama",
        Box::new(|cfg: &ProviderBuildConfig| {
            Arc::new(OllamaProvider::new(
                Some(cfg.api_key.clone()),
                cfg.base_url.clone(),
            )) as Arc<dyn Provider>
        }),
    );
    m.insert(
        "kimi",
        Box::new(|cfg: &ProviderBuildConfig| {
            Arc::new(KimiProvider::new(
                Some(cfg.api_key.clone()),
                cfg.base_url.clone(),
            )) as Arc<dyn Provider>
        }),
    );
    m.insert(
        "z_ai",
        Box::new(|cfg: &ProviderBuildConfig| {
            Arc::new(ZAIProvider::new(
                Some(cfg.api_key.clone()),
                cfg.base_url.clone(),
            )) as Arc<dyn Provider>
        }),
    );
    m.insert(
        "cloudflare_gateway",
        Box::new(|cfg: &ProviderBuildConfig| {
            Arc::new(CloudflareGatewayProvider::new(
                Some(cfg.api_key.clone()),
                cfg.base_url.clone(),
            )) as Arc<dyn Provider>
        }),
    );
    m.insert(
        "gemini",
        Box::new(|cfg: &ProviderBuildConfig| {
            Arc::new(GeminiProvider::new(
                Some(cfg.api_key.clone()),
                cfg.base_url.clone(),
            )) as Arc<dyn Provider>
        }),
    );
    m.insert(
        "openai",
        Box::new(|cfg: &ProviderBuildConfig| {
            Arc::new(OpenAIProvider::new(
                Some(cfg.api_key.clone()),
                cfg.base_url.clone(),
                None,
            )) as Arc<dyn Provider>
        }),
    );
    m.insert(
        "anthropic",
        Box::new(|cfg: &ProviderBuildConfig| {
            Arc::new(AnthropicProvider::new(
                Some(cfg.api_key.clone()),
                vec![],
                cfg.base_url.clone(),
            )) as Arc<dyn Provider>
        }),
    );
    m.insert(
        "siliconflow",
        Box::new(|cfg: &ProviderBuildConfig| {
            Arc::new(SiliconFlowProvider::new(
                Some(cfg.api_key.clone()),
                cfg.base_url.clone(),
            )) as Arc<dyn Provider>
        }),
    );
    m.insert(
        "fireworks",
        Box::new(|cfg: &ProviderBuildConfig| {
            Arc::new(FireworksProvider::new(
                Some(cfg.api_key.clone()),
                cfg.base_url.clone(),
            )) as Arc<dyn Provider>
        }),
    );

    m
}

/// Resolve the provider descriptor for a given id.
fn find_descriptor(provider_id: &str) -> Option<&'static ProviderDescriptor> {
    PROVIDER_CATALOG
        .iter()
        .find(|d| d.provider_id == provider_id)
}

/// Create a provider by id, building config from the descriptor and env.
pub fn create_provider(
    provider_id: &str,
    config: &EnvConfig,
) -> Result<Arc<dyn Provider>, ProviderError> {
    let descriptor =
        find_descriptor(provider_id).ok_or_else(|| {
            let supported: Vec<&str> = SUPPORTED_PROVIDER_IDS.to_vec();
            ProviderError::invalid_request(&format!(
                "Unknown provider_type: '{}'. Supported: {:?}",
                provider_id, supported
            ))
        })?;

    let provider_config = build_provider_config(descriptor, config)?;
    let factories = build_factory_map();
    let factory = factories.get(provider_id).ok_or_else(|| {
        ProviderError::internal(&format!("Unhandled provider descriptor: {}", provider_id))
    })?;
    Ok(factory(&provider_config))
}

// ---------------------------------------------------------------------------
// ProviderRegistry
// ---------------------------------------------------------------------------

/// Cache and clean up provider instances by provider id.
pub struct ProviderRegistry {
    providers: HashMap<String, Arc<dyn Provider>>,
    model_ids_by_provider: HashMap<String, HashSet<String>>,
    model_infos_by_provider: HashMap<String, HashMap<String, ProviderModelInfo>>,
}

impl ProviderRegistry {
    pub fn new() -> Self {
        Self {
            providers: HashMap::new(),
            model_ids_by_provider: HashMap::new(),
            model_infos_by_provider: HashMap::new(),
        }
    }

    /// Return whether a provider for this id is already in the cache.
    pub fn is_cached(&self, provider_id: &str) -> bool {
        self.providers.contains_key(provider_id)
    }

    /// Get or lazily instantiate a provider by id.
    pub fn get(
        &mut self,
        provider_id: &str,
        config: &EnvConfig,
    ) -> Result<Arc<dyn Provider>, ProviderError> {
        if let Some(p) = self.providers.get(provider_id) {
            return Ok(Arc::clone(p));
        }
        let provider = create_provider(provider_id, config)?;
        self.providers
            .insert(provider_id.to_string(), Arc::clone(&provider));
        Ok(provider)
    }

    /// Store a provider model-list result for later instant API responses.
    pub fn cache_model_ids(&mut self, provider_id: &str, model_ids: &[String]) {
        let infos = model_infos_from_ids(model_ids.iter().map(|s| s.as_str()), None);
        self.cache_model_infos(provider_id, infos);
    }

    /// Store provider model metadata for later instant API responses.
    pub fn cache_model_infos(
        &mut self,
        provider_id: &str,
        model_infos: Vec<ProviderModelInfo>,
    ) {
        let clean: HashMap<String, ProviderModelInfo> = model_infos
            .into_iter()
            .filter(|info| !info.model_id.trim().is_empty())
            .map(|info| (info.model_id.clone(), info))
            .collect();
        let ids: HashSet<String> = clean.keys().cloned().collect();
        self.model_infos_by_provider
            .insert(provider_id.to_string(), clean);
        self.model_ids_by_provider
            .insert(provider_id.to_string(), ids);
    }

    /// Return a copy of cached raw provider model ids.
    pub fn cached_model_ids(&self) -> &HashMap<String, HashSet<String>> {
        &self.model_ids_by_provider
    }

    /// Return cached thinking support when a provider exposes it.
    pub fn cached_model_supports_thinking(
        &self,
        provider_id: &str,
        model_id: &str,
    ) -> Option<bool> {
        self.model_infos_by_provider
            .get(provider_id)
            .and_then(|m| m.get(model_id))
            .and_then(|info| info.supports_thinking)
    }

    /// Return cached provider models with user-selectable `provider/model` prefixed ids.
    pub fn cached_prefixed_model_refs(&self) -> Vec<String> {
        self.cached_prefixed_model_infos()
            .into_iter()
            .map(|info| info.model_id)
            .collect()
    }

    /// Return cached provider models with user-selectable prefixed ids.
    pub fn cached_prefixed_model_infos(&self) -> Vec<ProviderModelInfo> {
        let mut infos: Vec<ProviderModelInfo> = Vec::new();
        for provider_id in SUPPORTED_PROVIDER_IDS {
            if let Some(provider_infos) = self.model_infos_by_provider.get(*provider_id) {
                let mut sorted: Vec<&ProviderModelInfo> = provider_infos.values().collect();
                sorted.sort_by(|a, b| a.model_id.cmp(&b.model_id));
                for info in sorted {
                    infos.push(ProviderModelInfo {
                        model_id: format!("{}/{}", provider_id, info.model_id),
                        supports_thinking: info.supports_thinking,
                    });
                }
            }
        }
        infos
    }

    /// Refresh model lists for providers usable in this process.
    pub async fn refresh_model_list_cache(
        &mut self,
        config: &EnvConfig,
        only_missing: bool,
    ) {
        let provider_ids = self.eligible_provider_ids(config, only_missing);
        self.refresh_model_ids(config, &provider_ids).await;
    }

    /// Validate that every configured chat model exists upstream.
    pub async fn validate_configured_models(
        &mut self,
        config: &EnvConfig,
        configured_refs: &[ConfiguredModelRef],
    ) -> Result<(), ProviderError> {
        let mut refs_by_provider: HashMap<String, Vec<&ConfiguredModelRef>> = HashMap::new();
        for ref_ in configured_refs {
            refs_by_provider
                .entry(ref_.provider_id.clone())
                .or_default()
                .push(ref_);
        }

        let mut failures: Vec<String> = Vec::new();
        // Collect (provider_id, model_ids result) after querying each provider.
        let mut query_results: Vec<(String, Result<Vec<String>, ProviderError>)> = Vec::new();

        for (provider_id, provider_refs) in &refs_by_provider {
            match self.get(provider_id, config) {
                Ok(provider) => {
                    let result = provider.list_models().await;
                    query_results.push((provider_id.clone(), result));
                }
                Err(exc) => {
                    for ref_ in provider_refs {
                        failures.push(format_model_validation_failure(
                            ref_,
                            &format!("provider init failure: {}", exc),
                        ));
                    }
                }
            }
        }

        for (provider_id, result) in &query_results {
            let provider_refs = refs_by_provider.get(provider_id).unwrap();
            match result {
                Ok(model_ids) => {
                    let id_set: HashSet<String> = model_ids.iter().cloned().collect();
                    self.cache_model_ids(provider_id, model_ids);
                    for ref_ in provider_refs {
                        if !id_set.contains(&ref_.model_id) {
                            failures.push(format_model_validation_failure(
                                ref_,
                                "missing model",
                            ));
                        }
                    }
                }
                Err(exc) => {
                    for ref_ in provider_refs {
                        failures.push(format_model_validation_failure(
                            ref_,
                            &format!("query failure: {}", exc),
                        ));
                    }
                }
            }
        }

        if !failures.is_empty() {
            let message = format!(
                "Configured model validation failed:\n{}",
                failures
                    .iter()
                    .map(|f| format!("- {}", f))
                    .collect::<Vec<_>>()
                    .join("\n")
            );
            return Err(ProviderError::internal(&message));
        }

        info!(
            "Configured provider models validated: models={} providers={}",
            configured_refs.len(),
            refs_by_provider.len()
        );
        Ok(())
    }

    /// Call cleanup on every cached provider, then clear the cache.
    pub fn clear(&mut self) {
        self.providers.clear();
        self.model_ids_by_provider.clear();
        self.model_infos_by_provider.clear();
    }

    // -- private helpers --

    /// Return provider ids worth discovering for this process configuration.
    fn eligible_provider_ids(&self, config: &EnvConfig, only_missing: bool) -> Vec<String> {
        let mut ids = Vec::new();
        for descriptor in PROVIDER_CATALOG {
            if descriptor.static_credential.is_some() {
                // Static credential providers are only included if referenced
                continue;
            }
            if descriptor.credential_env.is_some() {
                let cred = credential_for(descriptor, config);
                if !cred.trim().is_empty() {
                    if !only_missing || !self.model_ids_by_provider.contains_key(descriptor.provider_id) {
                        ids.push(descriptor.provider_id.to_string());
                    }
                }
            }
        }
        ids
    }

    /// Refresh model ids for the given providers concurrently.
    async fn refresh_model_ids(&mut self, config: &EnvConfig, provider_ids: &[String]) {
        // Collect results sequentially to satisfy borrow rules.
        let mut results: Vec<(String, Result<Vec<String>, ProviderError>)> = Vec::new();

        for provider_id in provider_ids {
            let result = match self.get(provider_id, config) {
                Ok(provider) => provider.list_models().await,
                Err(exc) => {
                    warn!(
                        "Provider model discovery skipped: provider={} reason={}",
                        provider_id, exc
                    );
                    continue;
                }
            };
            results.push((provider_id.clone(), result));
        }

        for (provider_id, result) in results {
            match result {
                Ok(model_ids) => {
                    let count = model_ids.len();
                    self.cache_model_ids(&provider_id, &model_ids);
                    info!(
                        "Provider model discovery cached: provider={} models={}",
                        provider_id, count
                    );
                }
                Err(exc) => {
                    warn!(
                        "Provider model discovery skipped: provider={} reason={}",
                        provider_id, exc
                    );
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Validation helpers
// ---------------------------------------------------------------------------

/// A reference to a configured model.
#[derive(Debug, Clone)]
pub struct ConfiguredModelRef {
    pub provider_id: String,
    pub model_id: String,
    pub sources: Vec<String>,
}

fn format_model_validation_failure(ref_: &ConfiguredModelRef, problem: &str) -> String {
    format!(
        "sources={} provider={} model={} problem={}",
        ref_.sources.join(","),
        ref_.provider_id,
        ref_.model_id,
        problem
    )
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_provider_config_static_credential() {
        let descriptor = find_descriptor("ollama").unwrap();
        let config = EnvConfig::from_env();
        let pc = build_provider_config(descriptor, &config).unwrap();
        assert_eq!(pc.api_key, "ollama");
    }

    #[test]
    fn test_find_descriptor() {
        assert!(find_descriptor("nvidia_nim").is_some());
        assert!(find_descriptor("nonexistent").is_none());
    }

    #[test]
    fn test_supported_provider_ids_count() {
        assert_eq!(SUPPORTED_PROVIDER_IDS.len(), 14);
    }

    #[test]
    fn test_format_model_validation_failure() {
        let ref_ = ConfiguredModelRef {
            provider_id: "openai".to_string(),
            model_id: "gpt-4".to_string(),
            sources: vec!["chat".to_string()],
        };
        let msg = format_model_validation_failure(&ref_, "missing model");
        assert!(msg.contains("openai"));
        assert!(msg.contains("gpt-4"));
        assert!(msg.contains("missing model"));
    }

    #[test]
    fn test_provider_registry_cache() {
        let mut registry = ProviderRegistry::new();
        assert!(!registry.is_cached("nvidia_nim"));
        registry.cache_model_ids("nvidia_nim", &["model-a".to_string(), "model-b".to_string()]);
        assert!(registry.cached_model_ids().contains_key("nvidia_nim"));
    }

    #[test]
    fn test_cached_prefixed_model_refs() {
        let mut registry = ProviderRegistry::new();
        registry.cache_model_ids("openai", &["gpt-4".to_string()]);
        registry.cache_model_ids("ollama", &["llama3".to_string()]);
        let refs = registry.cached_prefixed_model_refs();
        assert!(refs.contains(&"openai/gpt-4".to_string()));
        assert!(refs.contains(&"ollama/llama3".to_string()));
    }
}
