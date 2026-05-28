use axum::routing::{get, post};
use axum::Router;
use std::sync::Arc;

use crate::api::server::routes::{messages, models};
use crate::config::Config;

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
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
        let state = AppState {
            config: self.config.clone(),
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