use std::fs;
use std::path::{Path, PathBuf};
use texure_contract::config::{ResolvedDeviceConfig, TexureConfig};
use texure_contract::error::ErrorCode;
use tracing::warn;

/// Resolve default configuration file path for the current OS.
pub fn default_config_path() -> Option<PathBuf> {
    if let Ok(path_str) = std::env::var("TEXURE_CONFIG") {
        if !path_str.trim().is_empty() {
            return Some(PathBuf::from(path_str.trim()));
        }
    }
    dirs::config_dir().map(|dir| dir.join("texure").join("config.toml"))
}

/// Load configuration and resolve active target device parameters.
pub fn load_resolved_config(custom_path: Option<&Path>) -> Result<ResolvedDeviceConfig, ErrorCode> {
    let path = custom_path.map(PathBuf::from).or_else(default_config_path);

    let config = if let Some(ref p) = path {
        if p.exists() {
            match fs::read_to_string(p) {
                Ok(content) => match toml::from_str::<TexureConfig>(&content) {
                    Ok(cfg) => cfg,
                    Err(e) => {
                        warn!("Malformed config file {}: {e}; using defaults", p.display());
                        TexureConfig::default()
                    }
                },
                Err(e) => {
                    warn!(
                        "Unreadable config file {}: {e}; using defaults",
                        p.display()
                    );
                    TexureConfig::default()
                }
            }
        } else {
            TexureConfig::default()
        }
    } else {
        TexureConfig::default()
    };

    config.resolve_from_env()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_with_env_override() {
        std::env::set_var("TEXURE_URL", "http://127.0.0.1:8765");
        std::env::set_var("TEXURE_TOKEN", "test-token-123");

        let resolved = load_resolved_config(None).expect("resolved config");
        assert_eq!(resolved.url, "http://127.0.0.1:8765");
        assert_eq!(resolved.token, "test-token-123");

        std::env::remove_var("TEXURE_URL");
        std::env::remove_var("TEXURE_TOKEN");
    }
}
