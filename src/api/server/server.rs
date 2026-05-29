use std::sync::Arc;

use axum::extract::Request;
use axum::http::StatusCode;
use axum::middleware::{self, Next};
use axum::response::Response;
use axum::routing::{get, post};
use axum::{extract::State, Router};

use crate::api::server::routes::{
    count_tokens, create_message, get_model, get_model_capabilities, health, models,
    probe_count_tokens, probe_health, probe_messages, probe_root, root, web_fetch, web_search,
};
use crate::api::services::ClaudeProxyService;
use crate::config::Config;
use crate::providers::Provider;
use crate::providers::registry::EnvConfig;

fn make_provider_getter(
    config: Arc<Config>,
) -> Arc<dyn Fn(&str) -> Box<dyn Provider> + Send + Sync> {
    Arc::new(move |provider_id| match provider_id {
        "openai" => Box::new(crate::providers::OpenAIProvider::new(
            config.providers.openai.api_key.clone(),
            config.providers.openai.base_url.clone(),
            config.providers.openai.organization.clone(),
        )),
        "anthropic" => Box::new(crate::providers::AnthropicProvider::new(
            config.providers.anthropic.api_key.clone(),
            config.providers.anthropic.beta_features.clone(),
            config.providers.anthropic.base_url.clone(),
        )),
        "open_router" => Box::new(crate::providers::OpenRouterProvider::new(
            config.providers.open_router.api_key.clone(),
            config.providers.open_router.base_url.clone(),
            config.providers.open_router.referer.clone(),
        )),
        "nvidia_nim" => Box::new(crate::providers::NvidiaNimProvider::new(
            config.providers.nvidia_nim.base_url.clone(),
            config.providers.nvidia_nim.model_id.clone(),
            config.providers.nvidia_nim.api_key.clone(),
        )),
        "deepseek" => Box::new(crate::providers::DeepSeekProvider::new(
            config.providers.deepseek.api_key.clone(),
            config.providers.deepseek.base_url.clone(),
        )),
        "kimi" => Box::new(crate::providers::KimiProvider::new(
            config.providers.kimi.api_key.clone(),
            config.providers.kimi.base_url.clone(),
        )),
        "llamacpp" => Box::new(crate::providers::LlamaCppProvider::new(
            config.providers.llamacpp.base_url.clone(),
        )),
        "lmstudio" => Box::new(crate::providers::LmstudioProvider::new(
            config.providers.lmstudio.base_url.clone(),
        )),
        "fireworks" => Box::new(crate::providers::FireworksProvider::new(
            config.providers.fireworks.api_key.clone(),
            config.providers.fireworks.base_url.clone(),
        )),
        "siliconflow" => Box::new(crate::providers::SiliconFlowProvider::new(
            config.providers.siliconflow.api_key.clone(),
            config.providers.siliconflow.base_url.clone(),
        )),
        "z_ai" => Box::new(crate::providers::ZAIProvider::new(
            config.providers.z_ai.api_key.clone(),
            config.providers.z_ai.base_url.clone(),
        )),
        "gemini" => Box::new(crate::providers::GeminiProvider::new(
            config.providers.gemini.api_key.clone(),
            config.providers.gemini.base_url.clone(),
        )),
        "ollama" => Box::new(crate::providers::OllamaProvider::new(
            config.providers.ollama.api_key.clone(),
            config.providers.ollama.base_url.clone(),
        )),
        "cloudflare_gateway" => Box::new(
            crate::providers::CloudflareGatewayProvider::new(
                config.providers.cloudflare_gateway.api_key.clone(),
                config.providers.cloudflare_gateway.base_url.clone(),
            ),
        ),
        name => {
            tracing::warn!("Unknown provider: {name}, falling back to open_router");
            Box::new(crate::providers::OpenRouterProvider::new(
                config.providers.open_router.api_key.clone(),
                config.providers.open_router.base_url.clone(),
                config.providers.open_router.referer.clone(),
            ))
        }
    })
}

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub service: Arc<ClaudeProxyService>,
}

async fn auth_middleware(
    State(state): State<AppState>,
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let auth_token = &state.config.server.anthropic_auth_token;

    if auth_token.is_empty() {
        return Ok(next.run(request).await);
    }

    let headers = request.headers();

    let header_value = headers
        .get("x-api-key")
        .or_else(|| headers.get("authorization"))
        .or_else(|| headers.get("anthropic-auth-token"));

    let header_value = match header_value {
        Some(val) => val.to_str().unwrap_or(""),
        None => return Err(StatusCode::UNAUTHORIZED),
    };

    let mut token = header_value.to_string();

    if token.to_lowercase().starts_with("bearer ") {
        token = token.splitn(2, ' ').nth(1).unwrap_or("").to_string();
    }

    if let Some(idx) = token.find(':') {
        token = token[..idx].to_string();
    }

    if !constant_time_eq(token.as_bytes(), auth_token.as_bytes()) {
        return Err(StatusCode::UNAUTHORIZED);
    }

    Ok(next.run(request).await)
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }

    let mut result = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        result |= x ^ y;
    }
    result == 0
}

pub struct AppRuntime {
    config: Arc<Config>,
    provider_registry: Option<crate::providers::registry::ProviderRegistry>,
}

impl AppRuntime {
    pub fn new(config: Config) -> Self {
        Self {
            config: Arc::new(config),
            provider_registry: None,
        }
    }

    pub async fn startup(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        tracing::info!("Starting PxyClaude Proxy...");

        let mut registry = crate::providers::registry::ProviderRegistry::new();
        let env_config = EnvConfig::from_env();

        let configured_refs = self.build_configured_model_refs();
        registry.validate_configured_models(&env_config, &configured_refs).await?;

        self.provider_registry = Some(registry);

        tracing::info!("Startup complete");
        Ok(())
    }

    pub async fn shutdown(&mut self) {
        tracing::info!("Shutting down...");

        if let Some(mut registry) = self.provider_registry.take() {
            registry.clear();
        }

        tracing::info!("Shutdown complete");
    }

    pub fn build_configured_model_refs(&self) -> Vec<crate::providers::registry::ConfiguredModelRef> {
        let mut refs = Vec::new();

        let default_model = &self.config.providers.model;
        let provider_id = Config::parse_provider_type(default_model).to_string();
        let model_id = Config::parse_model_name(default_model).to_string();

        refs.push(crate::providers::registry::ConfiguredModelRef {
            provider_id,
            model_id,
            sources: vec!["MODEL".to_string()],
        });

        if let Some(ref opus) = self.config.providers.model_opus {
            let provider_id = Config::parse_provider_type(opus).to_string();
            let model_id = Config::parse_model_name(opus).to_string();
            refs.push(crate::providers::registry::ConfiguredModelRef {
                provider_id,
                model_id,
                sources: vec!["MODEL_OPUS".to_string()],
            });
        }

        if let Some(ref sonnet) = self.config.providers.model_sonnet {
            let provider_id = Config::parse_provider_type(sonnet).to_string();
            let model_id = Config::parse_model_name(sonnet).to_string();
            refs.push(crate::providers::registry::ConfiguredModelRef {
                provider_id,
                model_id,
                sources: vec!["MODEL_SONNET".to_string()],
            });
        }

        if let Some(ref haiku) = self.config.providers.model_haiku {
            let provider_id = Config::parse_provider_type(haiku).to_string();
            let model_id = Config::parse_model_name(haiku).to_string();
            refs.push(crate::providers::registry::ConfiguredModelRef {
                provider_id,
                model_id,
                sources: vec!["MODEL_HAIKU".to_string()],
            });
        }

        refs
    }

    pub async fn start_server(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let provider_getter = make_provider_getter(self.config.clone());
        let service = Arc::new(ClaudeProxyService::new(
            self.config.clone(),
            provider_getter,
        ));

        let state = AppState {
            config: self.config.clone(),
            service,
        };

        let app = Router::new()
            .route("/", get(root))
            .route("/", post(probe_root))
            .route("/health", get(health))
            .route("/health", post(probe_health))
            .route("/v1/models", get(models))
            .route("/v1/models/{model_id}", get(get_model))
            .route("/v1/models/{model_id}/capabilities", get(get_model_capabilities))
            .route("/v1/messages", post(create_message))
            .route("/v1/messages", get(probe_messages))
            .route("/v1/messages/count_tokens", post(count_tokens))
            .route("/v1/messages/count_tokens", get(probe_count_tokens))
            .route("/v1/web_search", post(web_search))
            .route("/v1/web_fetch", post(web_fetch))
            .layer(middleware::from_fn_with_state(
                state.clone(),
                auth_middleware,
            ))
            .with_state(state);

        let addr: std::net::SocketAddr = self.config.server.addr.parse()?;

        tracing::info!("Starting server on {}", addr);

        let listener = tokio::net::TcpListener::bind(addr).await?;

        axum::serve(listener, app)
            .await
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;

        Ok(())
    }
}

pub struct Server {
    config: Config,
}

impl Server {
    pub fn new(config: Config) -> Self {
        Self { config }
    }

    pub async fn start(self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut runtime = AppRuntime::new(self.config);

        runtime.startup().await?;

        let result = runtime.start_server().await;

        runtime.shutdown().await;

        result
    }
}
