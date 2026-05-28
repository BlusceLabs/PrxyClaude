use std::sync::Arc;

use axum::routing::{get, post};
use axum::Router;

use crate::api::server::routes::{messages, models};
use crate::api::services::ClaudeProxyService;
use crate::config::Config;
use crate::providers::Provider;

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

pub struct Server {
    config: Arc<Config>,
}

impl Server {
    pub fn new(config: Config) -> Self {
        Self {
            config: Arc::new(config),
        }
    }

    pub async fn start(self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
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
            .route("/v1/models", get(models))
            .route("/v1/messages", post(messages))
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
