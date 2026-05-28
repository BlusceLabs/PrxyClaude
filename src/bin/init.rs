//! PxyClaude init configuration binary

use clap::Parser;
use pxyclaude::config::Config;
use std::fs;
use std::path::Path;

#[derive(Parser, Debug)]
#[command(name = "proxycc-init")]
#[command(about = "Initialize PxyClaude configuration")]
pub struct InitCli {
    /// Configuration file path
    #[arg(short, long, default_value = "config.toml")]
    config: String,
    
    /// Overwrite existing config file
    #[arg(long)]
    force: bool,
    
    /// Create .env.example file
    #[arg(long)]
    env_example: bool,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = InitCli::parse();
    
    if Path::new(&cli.config).exists() && !cli.force {
        eprintln!("Config file {} already exists. Use --force to overwrite.", cli.config);
        std::process::exit(1);
    }
    
    let config_toml = Config::default().to_toml_string()?;
    fs::write(&cli.config, config_toml)?;
    println!("Created configuration file: {}", cli.config);
    
    if cli.env_example {
        let env_example = r#"# PxyClaude Environment Variables

# Server Configuration
PXCLAUDE_ADDR=127.0.0.1:8080
PXCLAUDE_LOG_LEVEL=info
PXCLAUDE_LOG_FILE=

# API Configuration
ENABLE_WEB_SERVER_TOOLS=true
WEB_FETCH_EGRESS_ALLOW_PRIVATE_NETWORK=false
WEB_FETCH_EGRESS_ALLOWED_SCHMES=["https"]
MAX_WEB_FETCH_CHARS=24000
MAX_WEB_SEARCH_RESULTS=10
MAX_WEB_FETCH_REDIRECTS=10
WEB_FETCH_REDIRECT_RESPONSE_BODY_CAP_BYTES=65536

# Provider Configuration
DEFAULT_PROVIDER=open_router
OPENAI_API_KEY=
OPENAI_BASE_URL=
ANTHROPIC_API_KEY=
OPEN_ROUTER_API_KEY=

# Logging Configuration
LOG_API_ERROR_TRACEBACKS=false
LOG_RAW_API_PAYLOADS=false

# Performance Configuration
MAX_CONCURRENT_REQUESTS=100
REQUEST_TIMEOUT_SECONDS=30
STREAMING_TIMEOUT_SECONDS=60

# Security Configuration
ENABLE_RATE_LIMITING=true
RATE_LIMIT_REQUESTS_PER_MINUTE=1000
ENABLE_CORS=true
CORS_ORIGINS=["http://localhost:3000", "http://localhost:8080"]

# Optional Features
ENABLE_VOICE=false
ENABLE_VOICE_LOCAL=false
"#;
        
        fs::write(".env.example", env_example)?;
        println!("Created .env.example file");
    }
    
    Ok(())
}
