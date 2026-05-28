//! PxyClaude server binary

use clap::Parser;
use pxyclaude::config::Config;
use pxyclaude::api::server::Server;
use tracing::info;

#[derive(Parser, Debug)]
#[command(name = "pxyclaude")]
#[command(about = "Middleware between Claude Code CLI (Anthropic API) and NVIDIA NIM")]
pub struct Cli {
    /// Configuration file path
    #[arg(short, long, default_value = "config.toml")]
    config: String,
    
    /// Listen address (host:port)
    #[arg(short, long, default_value = "127.0.0.1:8080")]
    addr: String,
    
    /// Enable debug logging
    #[arg(short, long)]
    debug: bool,
    
    /// Log level (trace, debug, info, warn, error)
    #[arg(long, default_value = "info")]
    log_level: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let cli = Cli::parse();
    
    let level = match cli.log_level.as_str() {
        "trace" => tracing::Level::TRACE,
        "debug" => tracing::Level::DEBUG,
        "info" => tracing::Level::INFO,
        "warn" => tracing::Level::WARN,
        "error" => tracing::Level::ERROR,
        _ => tracing::Level::INFO,
    };
    
    tracing_subscriber::fmt()
        .with_max_level(level)
        .with_target(false)
        .with_thread_ids(true)
        .init();
    
    info!("Starting PxyClaude server");
    
    let config = Config::load(&cli.config).unwrap_or_else(|_| Config::default());
    
    let mut final_config = config;
    if !cli.addr.is_empty() {
        final_config.server.addr = cli.addr;
    }
    
    let server = Server::new(final_config);
    server.start().await?;
    
    Ok(())
}
