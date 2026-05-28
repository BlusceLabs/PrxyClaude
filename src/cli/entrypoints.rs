use std::path::PathBuf;

use super::process_registry::kill_all_best_effort;

fn load_env_template() -> Result<String, String> {
    let source_template = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(".env.example");
    if source_template.is_file() {
        return std::fs::read_to_string(&source_template)
            .map_err(|e| format!("Failed to read .env.example: {}", e));
    }
    Err("Could not find .env.example template.".to_string())
}

pub fn serve() {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let _guard = runtime.enter();

    runtime.block_on(async {
        tracing::info!("Starting PxyClaude server...");

        let config = crate::config::Config::default();
        let server = crate::api::server::Server::new(config);
        server
            .start()
            .await
            .expect("Failed to start server");
    });

    kill_all_best_effort();
}

pub fn init() {
    let config_dir = home_config_dir().join("PxyClaude");
    let env_file = config_dir.join(".env");

    if env_file.exists() {
        println!("Config already exists at {}", env_file.display());
        println!("Delete it first if you want to reset to defaults.");
        return;
    }

    std::fs::create_dir_all(&config_dir).expect("Failed to create config directory");

    let template = load_env_template().expect("Failed to load .env.example");
    std::fs::write(&env_file, &template).expect("Failed to write .env file");

    println!("Config created at {}", env_file.display());
    println!("Edit it to set your API keys and model preferences, then run: PxyClaude");
}

fn home_config_dir() -> PathBuf {
    #[cfg(target_os = "linux")]
    {
        if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
            return PathBuf::from(xdg);
        }
    }
    if let Ok(home) = std::env::var("HOME") {
        #[cfg(target_os = "macos")]
        {
            return PathBuf::from(home).join("Library").join("Application Support");
        }
        #[cfg(target_os = "linux")]
        {
            return PathBuf::from(home).join(".config");
        }
    }
    if let Ok(profile) = std::env::var("USERPROFILE") {
        return PathBuf::from(profile);
    }
    PathBuf::from(".")
}
