use crate::error::ErrorCode;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A single device profile in the configuration file.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfileConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout_secs: Option<u64>,
}

/// Hybrid TOML configuration schema for texure per docs/lld-server.md §2.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TexureConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout_secs: Option<u64>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_profile: Option<String>,

    #[serde(default)]
    pub profiles: HashMap<String, ProfileConfig>,
}

/// Final resolved device connection parameters ready for client dial.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedDeviceConfig {
    pub url: String,
    pub token: String,
    pub timeout_secs: u64,
}

impl ResolvedDeviceConfig {
    /// Format base URL to guarantee no trailing slash.
    pub fn normalized_url(&self) -> &str {
        self.url.trim_end_matches('/')
    }
}

impl TexureConfig {
    /// Resolve effective target device config using precedence order:
    /// 1. Environment variables (`TEXURE_URL`, `TEXURE_TOKEN`, `TEXURE_TIMEOUT`)
    /// 2. Active profile in `[profiles.<active_profile>]`
    /// 3. Top-level TOML keys (`url`, `token`, `timeout_secs`)
    pub fn resolve<F>(&self, get_env: F) -> Result<ResolvedDeviceConfig, ErrorCode>
    where
        F: Fn(&str) -> Option<String>,
    {
        let env_url = get_env("TEXURE_URL").filter(|s| !s.trim().is_empty());
        let env_token = get_env("TEXURE_TOKEN").filter(|s| !s.trim().is_empty());
        let env_timeout = get_env("TEXURE_TIMEOUT").and_then(|s| s.parse::<u64>().ok());

        // Resolve profile if specified
        let profile = self
            .active_profile
            .as_deref()
            .and_then(|name| self.profiles.get(name));

        let url = env_url
            .or_else(|| profile.and_then(|p| p.url.clone()))
            .or_else(|| self.url.clone());

        let token = env_token
            .or_else(|| profile.and_then(|p| p.token.clone()))
            .or_else(|| self.token.clone());

        // timeout_secs = 0 would make reqwest's total timeout expire
        // immediately (every call fails as if the device were offline);
        // treat it as unset so the documented default applies.
        let timeout_secs = env_timeout
            .filter(|t| *t > 0)
            .or_else(|| profile.and_then(|p| p.timeout_secs).filter(|t| *t > 0))
            .or(self.timeout_secs)
            .filter(|t| *t > 0)
            .unwrap_or(10);

        match (url, token) {
            (Some(u), Some(t)) if !u.trim().is_empty() && !t.trim().is_empty() => {
                Ok(ResolvedDeviceConfig {
                    url: u.trim().to_string(),
                    token: t.trim().to_string(),
                    timeout_secs,
                })
            }
            _ => Err(ErrorCode::NoConfig),
        }
    }

    /// Resolve with an optional explicit profile override (CLI `--profile`),
    /// falling back to `TEXURE_PROFILE` env, then `active_profile` in file.
    /// A named profile that does not exist is a hard error (clear message).
    pub fn resolve_with_profile<F>(
        &self,
        profile_override: Option<&str>,
        get_env: F,
    ) -> Result<ResolvedDeviceConfig, (ErrorCode, Option<String>)>
    where
        F: Fn(&str) -> Option<String>,
    {
        let env_url = get_env("TEXURE_URL").filter(|s| !s.trim().is_empty());
        let env_token = get_env("TEXURE_TOKEN").filter(|s| !s.trim().is_empty());
        let env_timeout = get_env("TEXURE_TIMEOUT").and_then(|s| s.parse::<u64>().ok());

        let requested = profile_override
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .or_else(|| {
                get_env("TEXURE_PROFILE")
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
            });

        let profile = if let Some(name) = &requested {
            match self.profiles.get(name) {
                Some(p) => Some(p),
                None => {
                    return Err((
                        ErrorCode::NoConfig,
                        Some(format!("profile '{}' not found in config", name)),
                    ))
                }
            }
        } else {
            self.active_profile
                .as_deref()
                .and_then(|name| self.profiles.get(name))
        };

        let url = env_url
            .or_else(|| profile.and_then(|p| p.url.clone()))
            .or_else(|| self.url.clone());

        let token = env_token
            .or_else(|| profile.and_then(|p| p.token.clone()))
            .or_else(|| self.token.clone());

        let timeout_secs = env_timeout
            .filter(|t| *t > 0)
            .or_else(|| profile.and_then(|p| p.timeout_secs).filter(|t| *t > 0))
            .or(self.timeout_secs)
            .filter(|t| *t > 0)
            .unwrap_or(10);

        match (url, token) {
            (Some(u), Some(t)) if !u.trim().is_empty() && !t.trim().is_empty() => {
                Ok(ResolvedDeviceConfig {
                    url: u.trim().to_string(),
                    token: t.trim().to_string(),
                    timeout_secs,
                })
            }
            _ => Err((ErrorCode::NoConfig, None)),
        }
    }

    /// Resolve using standard process environment variables.
    pub fn resolve_from_env(&self) -> Result<ResolvedDeviceConfig, ErrorCode> {
        self.resolve(|key| std::env::var(key).ok())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_from_env_precedence() {
        let config = TexureConfig {
            url: Some("http://file-url:8765".to_string()),
            token: Some("file-token".to_string()),
            timeout_secs: Some(5),
            active_profile: None,
            profiles: HashMap::new(),
        };

        let resolved = config
            .resolve(|key| match key {
                "TEXURE_URL" => Some("http://env-url:8765".to_string()),
                _ => None,
            })
            .expect("resolve");

        assert_eq!(resolved.url, "http://env-url:8765");
        assert_eq!(resolved.token, "file-token");
        assert_eq!(resolved.timeout_secs, 5);
    }

    #[test]
    fn test_resolve_active_profile() {
        let mut profiles = HashMap::new();
        profiles.insert(
            "tablet".to_string(),
            ProfileConfig {
                url: Some("http://tablet:8765".to_string()),
                token: Some("tablet-token".to_string()),
                timeout_secs: Some(15),
            },
        );

        let config = TexureConfig {
            url: Some("http://default:8765".to_string()),
            token: Some("default-token".to_string()),
            timeout_secs: Some(10),
            active_profile: Some("tablet".to_string()),
            profiles,
        };

        let resolved = config.resolve(|_| None).expect("resolve");
        assert_eq!(resolved.url, "http://tablet:8765");
        assert_eq!(resolved.token, "tablet-token");
        assert_eq!(resolved.timeout_secs, 15);
    }

    #[test]
    fn test_missing_config_returns_no_config() {
        let config = TexureConfig::default();
        let res = config.resolve(|_| None);
        assert_eq!(res, Err(ErrorCode::NoConfig));
    }

    #[test]
    fn test_resolve_with_profile_override_selects_profile() {
        let mut profiles = HashMap::new();
        profiles.insert(
            "pixel".to_string(),
            ProfileConfig {
                url: Some("http://pixel:8765".to_string()),
                token: Some("pixel-token".to_string()),
                timeout_secs: Some(20),
            },
        );
        let config = TexureConfig {
            url: Some("http://default:8765".to_string()),
            token: Some("default-token".to_string()),
            timeout_secs: Some(10),
            active_profile: None,
            profiles,
        };
        let resolved = config
            .resolve_with_profile(Some("pixel"), |_| None)
            .expect("resolve");
        assert_eq!(resolved.url, "http://pixel:8765");
        assert_eq!(resolved.token, "pixel-token");
        assert_eq!(resolved.timeout_secs, 20);
    }

    #[test]
    fn test_resolve_with_profile_override_missing_is_error() {
        let config = TexureConfig {
            url: Some("http://default:8765".to_string()),
            token: Some("default-token".to_string()),
            timeout_secs: Some(10),
            active_profile: None,
            profiles: HashMap::new(),
        };
        let res = config.resolve_with_profile(Some("nope"), |_| None);
        assert!(res.is_err());
        let (code, msg) = res.unwrap_err();
        assert_eq!(code, ErrorCode::NoConfig);
        assert!(msg.unwrap_or_default().contains("not found"));
    }

    #[test]
    fn test_resolve_with_texure_profile_env() {
        let mut profiles = HashMap::new();
        profiles.insert(
            "tablet".to_string(),
            ProfileConfig {
                url: Some("http://tablet:8765".to_string()),
                token: Some("tablet-token".to_string()),
                timeout_secs: Some(15),
            },
        );
        let config = TexureConfig {
            url: Some("http://default:8765".to_string()),
            token: Some("default-token".to_string()),
            timeout_secs: Some(10),
            active_profile: None,
            profiles,
        };
        let resolved = config
            .resolve_with_profile(None, |key| {
                if key == "TEXURE_PROFILE" {
                    Some("tablet".to_string())
                } else {
                    None
                }
            })
            .expect("resolve");
        assert_eq!(resolved.url, "http://tablet:8765");
    }

    #[test]
    fn test_resolve_with_profile_fallback_to_top_level() {
        let config = TexureConfig {
            url: Some("http://default:8765".to_string()),
            token: Some("default-token".to_string()),
            timeout_secs: Some(10),
            active_profile: None,
            profiles: HashMap::new(),
        };
        let resolved = config
            .resolve_with_profile(None, |_| None)
            .expect("resolve");
        assert_eq!(resolved.url, "http://default:8765");
        assert_eq!(resolved.token, "default-token");
    }

    #[test]
    fn test_zero_timeout_falls_back_to_default() {
        let config = TexureConfig {
            url: Some("http://device:8765".to_string()),
            token: Some("tok".to_string()),
            timeout_secs: Some(0),
            active_profile: None,
            profiles: HashMap::new(),
        };

        let resolved = config
            .resolve(|key| (key == "TEXURE_TIMEOUT").then(|| "0".to_string()))
            .expect("resolve");
        assert_eq!(resolved.timeout_secs, 10);
    }
}
