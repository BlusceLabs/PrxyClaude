use std::path::Path;
use std::sync::OnceLock;

static CONFIGURED: OnceLock<bool> = OnceLock::new();

pub fn configure_logging(log_file: &str, verbose_third_party: bool) {
    if CONFIGURED.get().is_some() {
        return;
    }

    let path = Path::new(log_file);
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    tracing_subscriber::fmt()
        .with_target(true)
        .with_level(true)
        .with_file(true)
        .with_line_number(true)
        .json()
        .with_max_level(if verbose_third_party {
            tracing::Level::DEBUG
        } else {
            tracing::Level::WARN
        })
        .init();

    let _ = CONFIGURED.set(true);
}
