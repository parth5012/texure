//! Configurable transport resolution (Group R, ticket #62).
//!
//! Tool calls evaluate `[network].transport_order` in strict declared order
//! and use the first transport that can serve them. The default order
//! (`reverse_ws -> lan_direct -> tailscale`) keeps the pre-#62 observable
//! behavior: a connected companion wins, otherwise the direct LAN bridge is
//! used. Tailscale is only considered when a destination is configured.
//!
//! The deep seam here is [`resolve_with`]: priority/fallback/error semantics
//! are a pure loop over a probe closure, so unit tests exercise the real
//! resolution logic without a tunnel or a device. The async adapter
//! [`resolve_transport`] supplies the three concrete probes.

use crate::bridge_client::BridgeClient;
use crate::config::load_resolved_config_async;
use crate::tunnel_server::TunnelHandle;
use std::future::Future;
use std::path::Path;
use texure_contract::config::{ResolvedDeviceConfig, TexureConfig};
use texure_contract::error::ErrorCode;

/// A transport named in `[network].transport_order`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Transport {
    ReverseWs,
    LanDirect,
    Tailscale,
}

/// A resolution failure: one frozen error code plus a clear, one-line message.
/// No new error codes are introduced (G8).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolutionFailure {
    pub code: ErrorCode,
    pub message: String,
}

impl ResolutionFailure {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    /// The full `CODE: message; hint` text returned to MCP clients.
    pub fn one_line(&self) -> String {
        self.code.formatted_single_line(Some(&self.message))
    }
}

/// The resource a tool call uses after resolution.
pub enum SelectedTransport {
    /// A connected reverse-WS companion (routed via `TunnelRouter`).
    ReverseWs(TunnelHandle),
    /// A direct bridge client: LAN URL or Tailscale URL, already validated.
    Direct(BridgeClient),
}

impl std::fmt::Debug for SelectedTransport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ReverseWs(_) => f.write_str("ReverseWs(<tunnel>)"),
            Self::Direct(client) => f.debug_tuple("Direct").field(&client.config.url).finish(),
        }
    }
}

/// Parse and validate configured transport names. Unknown names and an empty
/// order are config errors (frozen `NO_CONFIG`) rather than silent fallbacks.
pub fn parse_transport_order(names: &[String]) -> Result<Vec<Transport>, ResolutionFailure> {
    if names.is_empty() {
        return Err(ResolutionFailure::new(
            ErrorCode::NoConfig,
            "[network].transport_order must not be empty",
        ));
    }
    names
        .iter()
        .map(|name| match name.trim() {
            "reverse_ws" => Ok(Transport::ReverseWs),
            "lan_direct" => Ok(Transport::LanDirect),
            "tailscale" => Ok(Transport::Tailscale),
            other => Err(ResolutionFailure::new(
                ErrorCode::NoConfig,
                format!(
                    "invalid transport '{other}' in [network].transport_order (valid: reverse_ws, lan_direct, tailscale)"
                ),
            )),
        })
        .collect()
}

/// Evaluate transports in declared order, returning the first resource whose
/// probe succeeds. Failures are collected for the all-fail error: a
/// `NO_CONFIG` failure wins over `OFFLINE` so "no config, no tunnel" keeps the
/// behavior-spec `NO_CONFIG` result; otherwise all-fail is `OFFLINE`.
pub async fn resolve_with<F, Fut, R>(
    order: &[Transport],
    mut probe: F,
) -> Result<R, ResolutionFailure>
where
    F: FnMut(Transport) -> Fut,
    Fut: Future<Output = Result<R, ResolutionFailure>>,
{
    if order.is_empty() {
        return Err(ResolutionFailure::new(
            ErrorCode::Offline,
            "no transports configured",
        ));
    }

    let mut last_failure = None;
    let mut no_config_failure = None;
    for &transport in order {
        match probe(transport).await {
            Ok(resource) => return Ok(resource),
            Err(failure) => {
                if failure.code == ErrorCode::NoConfig && no_config_failure.is_none() {
                    no_config_failure = Some(failure.clone());
                }
                last_failure = Some(failure);
            }
        }
    }

    Err(no_config_failure.or(last_failure).unwrap_or_else(|| {
        ResolutionFailure::new(ErrorCode::Offline, "all transports unavailable")
    }))
}

/// Build the dial config for a Tailscale destination, if one is configured.
/// Reuses the resolved token/timeout; only the URL changes.
pub fn tailscale_config(
    base: &ResolvedDeviceConfig,
    host: Option<&str>,
) -> Option<ResolvedDeviceConfig> {
    let host = host?.trim();
    // The bridge port is fixed at 8765; reject embedded schemes/ports so a
    // value like `100.64.0.5:9999` cannot produce a malformed URL.
    if host.is_empty()
        || host.contains("://")
        || host.contains(':')
        || host.contains(char::is_whitespace)
    {
        return None;
    }
    let host = host.trim_matches('/');
    if host.is_empty() {
        return None;
    }
    let mut config = base.clone();
    config.url = format!("http://{host}:8765");
    Some(config)
}

async fn resolve_lan(
    path: Option<&Path>,
    profile: Option<&str>,
) -> Result<ResolvedDeviceConfig, ResolutionFailure> {
    load_resolved_config_async(path, profile)
        .await
        .map_err(|(code, msg)| {
            ResolutionFailure::new(
                code,
                msg.unwrap_or_else(|| code.default_message().to_string()),
            )
        })
}

/// Tailscale overrides the URL, so resolve the base config without the mDNS
/// `url="auto"` discovery step (which would be pointless for a fixed tailnet
/// destination). Env/profile precedence is unchanged.
fn resolve_tailscale_base(
    config: &TexureConfig,
    profile: Option<&str>,
) -> Result<ResolvedDeviceConfig, ResolutionFailure> {
    config
        .resolve_with_profile(profile, |key| std::env::var(key).ok())
        .map_err(|(code, msg)| {
            ResolutionFailure::new(
                code,
                msg.unwrap_or_else(|| code.default_message().to_string()),
            )
        })
}

fn client_for(config: ResolvedDeviceConfig) -> Result<BridgeClient, ResolutionFailure> {
    BridgeClient::new(config).map_err(|e| ResolutionFailure::new(e.code, e.message))
}

/// Resolve the configured order against live availability. Called lazily on
/// every tool call so config edits and tunnel state changes take effect.
pub async fn resolve_transport(
    tunnel: Option<&TunnelHandle>,
    custom_config_path: Option<&Path>,
    profile_override: Option<&str>,
) -> Result<SelectedTransport, ResolutionFailure> {
    resolve_transport_excluding(tunnel, custom_config_path, profile_override, None).await
}

/// Same as [`resolve_transport`], but skips one transport. Used after a tunnel
/// transport failure so the next configured candidate actually gets tried
/// (`transport_order.json` `fallback_on`: tunnel_unreachable / heartbeat_timeout).
pub async fn resolve_transport_excluding(
    tunnel: Option<&TunnelHandle>,
    custom_config_path: Option<&Path>,
    profile_override: Option<&str>,
    exclude: Option<Transport>,
) -> Result<SelectedTransport, ResolutionFailure> {
    let config = crate::config::read_config(custom_config_path);
    let order = parse_transport_order(&config.network.transport_order)?;
    let tailscale_ip = config.network.tailscale_ip.clone();

    let tunnel_ref = tunnel;
    let path_ref = custom_config_path;
    let profile_ref = profile_override;
    let config_ref = &config;

    let mut probe = |transport| {
        let tunnel = tunnel_ref;
        let path = path_ref;
        let profile = profile_ref;
        let config = config_ref;
        let tailscale_ip = tailscale_ip.clone();
        async move {
            if exclude == Some(transport) {
                return Err(ResolutionFailure::new(
                    ErrorCode::Offline,
                    "transport excluded after failure",
                ));
            }
            match transport {
                Transport::ReverseWs => match tunnel.filter(|handle| handle.is_connected()) {
                    Some(handle) => Ok(SelectedTransport::ReverseWs(handle.clone())),
                    None => Err(ResolutionFailure::new(
                        ErrorCode::Offline,
                        "reverse_ws tunnel not connected",
                    )),
                },
                Transport::LanDirect => {
                    let resolved = resolve_lan(path, profile).await?;
                    Ok(SelectedTransport::Direct(client_for(resolved)?))
                }
                Transport::Tailscale => {
                    let host = tailscale_ip
                        .as_deref()
                        .filter(|host| !host.trim().is_empty())
                        .ok_or_else(|| {
                            ResolutionFailure::new(
                                ErrorCode::Offline,
                                "tailscale destination not configured",
                            )
                        })?;
                    let base = resolve_tailscale_base(config, profile)?;
                    let target = tailscale_config(&base, Some(host)).ok_or_else(|| {
                        ResolutionFailure::new(
                            ErrorCode::NoConfig,
                            format!(
                                "invalid tailscale_ip '{host}' in [network]: expected a bare IP or hostname (no scheme or port)"
                            ),
                        )
                    })?;
                    Ok(SelectedTransport::Direct(client_for(target)?))
                }
            }
        }
    };

    resolve_with(&order, &mut probe).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use texure_contract::config::ResolvedDeviceConfig;
    use texure_contract::error::ErrorCode;

    fn resolved(url: &str) -> ResolvedDeviceConfig {
        ResolvedDeviceConfig {
            url: url.to_string(),
            token: "tok".to_string(),
            timeout_secs: 10,
        }
    }

    #[test]
    fn parse_transport_order_accepts_known_names_in_declared_order() {
        let order = parse_transport_order(&[
            "tailscale".to_string(),
            "reverse_ws".to_string(),
            "lan_direct".to_string(),
        ])
        .expect("valid order");
        assert_eq!(
            order,
            vec![
                Transport::Tailscale,
                Transport::ReverseWs,
                Transport::LanDirect
            ]
        );
    }

    #[test]
    fn parse_transport_order_rejects_unknown_name() {
        let err = parse_transport_order(&["reverse_ws".to_string(), "bogus".to_string()])
            .expect_err("invalid name must be a config error");
        assert_eq!(err.code, ErrorCode::NoConfig);
        assert!(err.message.contains("bogus"));
        assert!(err.message.contains("reverse_ws"));
    }

    #[test]
    fn parse_transport_order_rejects_empty_order() {
        let err = parse_transport_order(&[]).expect_err("empty order must be a config error");
        assert_eq!(err.code, ErrorCode::NoConfig);
        assert!(err.message.contains("empty"));
    }

    #[test]
    fn default_transport_order_is_reverse_ws_first() {
        let order = parse_transport_order(
            &texure_contract::config::NetworkConfig::default().transport_order,
        )
        .expect("default order is valid");
        assert_eq!(
            order,
            vec![
                Transport::ReverseWs,
                Transport::LanDirect,
                Transport::Tailscale
            ]
        );
    }

    #[tokio::test]
    async fn resolve_with_selects_first_available_in_declared_order() {
        let order = vec![
            Transport::ReverseWs,
            Transport::LanDirect,
            Transport::Tailscale,
        ];
        let selected = resolve_with(&order, |transport| async move { Ok(transport) })
            .await
            .expect("a transport is available");
        assert_eq!(selected, Transport::ReverseWs);
    }

    #[tokio::test]
    async fn resolve_with_honors_custom_order_tailscale_first() {
        let order = parse_transport_order(&[
            "tailscale".to_string(),
            "lan_direct".to_string(),
            "reverse_ws".to_string(),
        ])
        .expect("valid order");
        let selected = resolve_with(&order, |transport| async move { Ok(transport) })
            .await
            .expect("a transport is available");
        assert_eq!(selected, Transport::Tailscale);
    }

    #[tokio::test]
    async fn resolve_with_falls_back_when_preferred_fails() {
        let order = vec![Transport::ReverseWs, Transport::LanDirect];
        let selected = resolve_with(&order, |transport| async move {
            match transport {
                Transport::ReverseWs => {
                    Err(ResolutionFailure::new(ErrorCode::Offline, "tunnel absent"))
                }
                _ => Ok(transport),
            }
        })
        .await
        .expect("second transport is available");
        assert_eq!(selected, Transport::LanDirect);
    }

    #[tokio::test]
    async fn resolve_with_all_fail_returns_offline_one_liner() {
        let order = vec![
            Transport::ReverseWs,
            Transport::LanDirect,
            Transport::Tailscale,
        ];
        let err = resolve_with(&order, |_| async move {
            Err::<(), _>(ResolutionFailure::new(ErrorCode::Offline, "down"))
        })
        .await
        .expect_err("all transports failed");
        assert_eq!(err.code, ErrorCode::Offline);
        let line = err.one_line();
        assert!(line.starts_with("OFFLINE: "), "{line}");
        assert!(line.contains("; "), "hint must be present: {line}");
        assert!(!line.contains('\n'), "hint must be one line: {line}");
    }

    #[tokio::test]
    async fn resolve_with_preserves_no_config_over_offline() {
        // No config + no tunnel must keep the current NO_CONFIG behavior.
        let order = vec![
            Transport::ReverseWs,
            Transport::LanDirect,
            Transport::Tailscale,
        ];
        let err = resolve_with(&order, |transport| async move {
            match transport {
                Transport::ReverseWs => {
                    Err::<Transport, _>(ResolutionFailure::new(ErrorCode::Offline, "tunnel absent"))
                }
                Transport::LanDirect => {
                    Err(ResolutionFailure::new(ErrorCode::NoConfig, "no config"))
                }
                Transport::Tailscale => Err(ResolutionFailure::new(
                    ErrorCode::Offline,
                    "tailscale absent",
                )),
            }
        })
        .await
        .expect_err("all transports failed");
        assert_eq!(err.code, ErrorCode::NoConfig);
        assert_eq!(err.message, "no config");
    }

    #[test]
    fn tailscale_config_none_when_unconfigured() {
        let base = resolved("http://lan:8765");
        assert!(tailscale_config(&base, None).is_none());
        assert!(tailscale_config(&base, Some("   ")).is_none());
        assert!(tailscale_config(&base, Some("100.64.0.5:9999")).is_none());
    }

    #[test]
    fn tailscale_config_uses_tailscale_host_when_configured() {
        let base = resolved("http://lan:8765");
        let config = tailscale_config(&base, Some("100.64.0.5")).expect("tailscale config");
        assert_eq!(config.url, "http://100.64.0.5:8765");
        assert_eq!(config.token, "tok");
        assert_eq!(config.timeout_secs, 10);
    }

    fn temp_config(contents: &str, tag: &str) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!(
            "texure-transport-{tag}-{}.toml",
            std::process::id()
        ));
        std::fs::write(&path, contents).expect("write test config");
        path
    }

    fn current_thread_runtime() -> tokio::runtime::Runtime {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime")
    }

    #[test]
    fn resolve_transport_selects_tailscale_only_when_configured() {
        let _env = crate::test_env_lock::lock();
        std::env::set_var("TEXURE_URL", "http://lan:8765");
        std::env::set_var("TEXURE_TOKEN", "tok");
        let rt = current_thread_runtime();

        let without = temp_config("[network]\ntransport_order = ['tailscale']\n", "no-ts");
        let err = rt
            .block_on(resolve_transport(None, Some(&without), None))
            .expect_err("tailscale is not configured");
        assert_eq!(err.code, ErrorCode::Offline);
        assert!(err.message.contains("not configured"));

        let with = temp_config(
            "[network]\ntransport_order = ['tailscale']\ntailscale_ip = '100.64.0.5'\n",
            "ts",
        );
        let selected = rt
            .block_on(resolve_transport(None, Some(&with), None))
            .expect("tailscale selected");
        match selected {
            SelectedTransport::Direct(client) => {
                assert_eq!(client.config.url, "http://100.64.0.5:8765");
                assert_eq!(client.config.token, "tok");
            }
            SelectedTransport::ReverseWs(_) => panic!("expected a direct tailscale client"),
        }

        std::env::remove_var("TEXURE_URL");
        std::env::remove_var("TEXURE_TOKEN");
        let _ = std::fs::remove_file(without);
        let _ = std::fs::remove_file(with);
    }

    #[test]
    fn resolve_transport_no_config_no_tunnel_is_no_config() {
        let _env = crate::test_env_lock::lock();
        std::env::remove_var("TEXURE_URL");
        std::env::remove_var("TEXURE_TOKEN");
        std::env::remove_var("TEXURE_PROFILE");
        let rt = current_thread_runtime();

        let err = rt
            .block_on(resolve_transport(
                None,
                Some(std::path::Path::new("/nonexistent/texure-transport.toml")),
                None,
            ))
            .expect_err("no device config");
        assert_eq!(err.code, ErrorCode::NoConfig);
    }
}
