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
/// Read + parse the TOML config file (or default when absent/malformed),
/// applying the default config path when `custom_path` is `None`.
pub fn read_config(custom_path: Option<&Path>) -> TexureConfig {
    let path = custom_path.map(PathBuf::from).or_else(default_config_path);

    if let Some(ref p) = path {
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
    }
}

pub fn load_resolved_config(custom_path: Option<&Path>) -> Result<ResolvedDeviceConfig, ErrorCode> {
    read_config(custom_path).resolve_from_env()
}

/// Load and resolve config, resolving `url="auto"` via mDNS LAN discovery
/// (map #36, ticket #32). Omitted/empty URLs keep existing semantics:
/// `resolve` yields `NO_CONFIG` before discovery is considered. When
/// discovery finds nothing within the timeout (multicast blocked / AP
/// isolation), the caller-visible error is `OFFLINE` — the device is
/// unreachable, same as a static-IP dial failure. No new error code.
/// Load + resolve config, applying an optional `--profile` override and the
/// `TEXURE_PROFILE` env var (env takes precedence over the CLI flag, which
/// takes precedence over `active_profile` in the file). Resolves `url="auto"`
/// via mDNS. Returns `(ErrorCode, Option<message>)` so a missing named
/// profile yields a clear error without a new error code (frozen 16).
pub async fn load_resolved_config_async(
    custom_path: Option<&Path>,
    profile_override: Option<&str>,
) -> Result<ResolvedDeviceConfig, (ErrorCode, Option<String>)> {
    let config = read_config(custom_path);
    let mut resolved = config
        .resolve_with_profile(profile_override, |key| std::env::var(key).ok())
        .map_err(|(code, msg)| (code, msg))?;
    if crate::mdns::is_auto_url(&resolved.url) {
        match crate::mdns::discover().await {
            Some(service) => {
                resolved.url = service.resolved_url();
            }
            None => return Err((ErrorCode::Offline, None)),
        }
    }
    Ok(resolved)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_with_env_override() {
        let _env = crate::test_env_lock::lock();
        std::env::set_var("TEXURE_URL", "http://127.0.0.1:8765");
        std::env::set_var("TEXURE_TOKEN", "test-token-123");

        let resolved = load_resolved_config(None).expect("resolved config");
        assert_eq!(resolved.url, "http://127.0.0.1:8765");
        assert_eq!(resolved.token, "test-token-123");

        std::env::remove_var("TEXURE_URL");
        std::env::remove_var("TEXURE_TOKEN");
    }

    // Sync + block_on so the env lock is never held across an await
    // (clippy await_holding_lock; same pattern as server.rs tests).
    #[test]
    fn test_auto_url_without_responder_is_offline() {
        // `url="auto"` with no LAN responder (unit-test env) falls back to
        // OFFLINE semantics; omitted url (NoConfig) is covered in
        // texure-contract config tests and NOT flipped by this ticket.
        let _env = crate::test_env_lock::lock();
        std::env::set_var("TEXURE_URL", "auto");
        std::env::set_var("TEXURE_TOKEN", "test-token-123");

        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime");
        let res = rt.block_on(async {
            load_resolved_config_async(Some(Path::new("/nonexistent/texure_config.toml")), None).await
        });

        std::env::remove_var("TEXURE_URL");
        std::env::remove_var("TEXURE_TOKEN");
        assert_eq!(res.map_err(|(c, _)| c), Err(ErrorCode::Offline));
    }

    #[test]
    fn test_profile_override_missing_is_error() {
        let _env = crate::test_env_lock::lock();
        std::env::set_var("TEXURE_URL", "http://default:8765");
        std::env::set_var("TEXURE_TOKEN", "default-token");

        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime");
        let res = rt.block_on(async {
            load_resolved_config_async(
                Some(Path::new("/nonexistent/texure_config.toml")),
                Some("nope"),
            )
            .await
        });

        std::env::remove_var("TEXURE_URL");
        std::env::remove_var("TEXURE_TOKEN");
        assert!(res.is_err());
        let (code, msg) = res.unwrap_err();
        assert_eq!(code, ErrorCode::NoConfig);
        assert!(msg.unwrap_or_default().contains("not found"));
    }

    #[test]
    fn test_static_url_skips_discovery() {
        let _env = crate::test_env_lock::lock();
        std::env::set_var("TEXURE_URL", "http://127.0.0.1:8765");
        std::env::set_var("TEXURE_TOKEN", "test-token-123");

        let resolved = load_resolved_config(Some(Path::new("/nonexistent/texure_config.toml")))
            .expect("resolved config");

        std::env::remove_var("TEXURE_URL");
        std::env::remove_var("TEXURE_TOKEN");
        assert_eq!(resolved.url, "http://127.0.0.1:8765");
    }
}
