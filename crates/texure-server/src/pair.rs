//! `texure-server pair` — render a scannable ASCII QR plus the raw `wss://`
//! connection string for pairing the remote companion (BB-R11, Group R).
//!
//! The QR payload mirrors `contract/fixtures/pairing_qr_payload.json` so the
//! companion parser (`companion/src/lib/qr-parser.ts`) validates it. No new
//! MCP tools are introduced (guardrail G7); this is a CLI-only subcommand.

use std::net::IpAddr;
use std::path::Path;
use texure_contract::config::TexureConfig;

/// Default tunnel port on the detected LAN IP (ticket #60 reuses this).
pub const DEFAULT_TUNNEL_PORT: u16 = 8766;
/// Default tunnel path (ticket #60 reuses this).
pub const DEFAULT_TUNNEL_PATH: &str = "/tunnel";
/// Default `wss://` port when a custom tunnel override has no explicit port.
pub const DEFAULT_WSS_PORT: u16 = 443;
/// App identifier embedded in the pairing payload.
pub const QR_APP: &str = "texure";

/// A normalized `--tunnel-url` override: host, port, path (scheme always wss).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TunnelOverride {
    pub host: String,
    pub port: u16,
    pub path: String,
}

/// Where the pairing token came from (reused config vs. fresh fallback).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenSource {
    Config,
    Generated,
}

/// Fully resolved pairing endpoint used to build the connection string + QR.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PairingEndpoint {
    pub host: String,
    pub port: u16,
    pub path: String,
    pub token: String,
}

/// Percent-encode a value for a URL query parameter. Unreserved characters
/// (including the base64url token alphabet) are left untouched; everything
/// else is encoded so a configured token like `a+b` survives companion parsing.
fn encode_query_value(value: &str) -> String {
    let mut encoded = String::with_capacity(value.len());
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(byte as char)
            }
            _ => encoded.push_str(&format!("%{byte:02X}")),
        }
    }
    encoded
}

impl PairingEndpoint {
    /// Human-readable raw connection string: `wss://host:port/path?token=…`.
    pub fn connection_string(&self) -> String {
        format!(
            "wss://{}:{}{}?token={}",
            self.host,
            self.port,
            self.path,
            encode_query_value(&self.token)
        )
    }

    /// JSON payload matching `contract/fixtures/pairing_qr_payload.json`.
    pub fn qr_payload_json(&self) -> String {
        serde_json::json!({
            "scheme": "wss",
            "host": self.host,
            "port": self.port,
            "path": self.path,
            "token": self.token,
            "token_present": true,
            "state": "UNPAIRED",
            "app": QR_APP,
        })
        .to_string()
    }
}

/// Normalize a `--tunnel-url` value. Accepts a full `wss://host[:port]/path`
/// or a bare `host[:port][/path]`; scheme is forced to `wss`.
pub fn normalize_tunnel_url(raw: &str) -> Result<TunnelOverride, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err("tunnel URL must not be empty".to_string());
    }

    // Drop any scheme (wss://, https://, ws:// …); the pairing transport is
    // always wss regardless of what the user pasted.
    let authority_and_path = match trimmed.find("://") {
        Some(idx) => &trimmed[idx + 3..],
        None => trimmed,
    };
    if authority_and_path.starts_with('/') {
        return Err(format!("tunnel URL '{raw}' is missing a host"));
    }

    // The authority ends at the first path/query/fragment delimiter; leaving
    // `?` or `#` attached would put the token into a fragment.
    let (authority, raw_path) = match authority_and_path.find(['/', '?', '#']) {
        Some(idx) => (&authority_and_path[..idx], &authority_and_path[idx..]),
        None => (authority_and_path, DEFAULT_TUNNEL_PATH),
    };

    let (host, port) = if let Some(inner) = authority.strip_prefix('[') {
        // Bracketed IPv6: [::1] or [::1]:8443
        let end = inner
            .find(']')
            .ok_or_else(|| format!("malformed IPv6 host in tunnel URL '{raw}'"))?;
        let host = format!("[{}]", &inner[..end]);
        let remainder = &inner[end + 1..];
        let port = parse_optional_port(remainder, raw)?;
        (host, port)
    } else if let Some((name, port_str)) = authority.rsplit_once(':') {
        let port = port_str
            .parse::<u16>()
            .map_err(|_| format!("invalid port '{port_str}' in tunnel URL '{raw}'"))?;
        (name.to_string(), Some(port))
    } else {
        (authority.to_string(), None)
    };

    if host.is_empty() {
        return Err(format!("tunnel URL '{raw}' is missing a host"));
    }
    if host.chars().any(char::is_whitespace) {
        return Err(format!("tunnel URL '{raw}' has an invalid host"));
    }
    if port == Some(0) {
        return Err(format!("invalid port 0 in tunnel URL '{raw}'"));
    }

    // Strip any query/fragment so the builder can append `?token=` cleanly.
    let clean_path: &str = raw_path.split(['?', '#']).next().unwrap_or(raw_path);
    let path = if clean_path.is_empty() {
        DEFAULT_TUNNEL_PATH.to_string()
    } else if clean_path.starts_with('/') {
        clean_path.to_string()
    } else {
        format!("/{clean_path}")
    };

    Ok(TunnelOverride {
        host,
        port: port.unwrap_or(DEFAULT_WSS_PORT),
        path,
    })
}

fn parse_optional_port(remainder: &str, raw: &str) -> Result<Option<u16>, String> {
    if remainder.is_empty() {
        return Ok(None);
    }
    let port_str = remainder
        .strip_prefix(':')
        .ok_or_else(|| format!("malformed host in tunnel URL '{raw}'"))?;
    let port = port_str
        .parse::<u16>()
        .map_err(|_| format!("invalid port '{port_str}' in tunnel URL '{raw}'"))?;
    Ok(Some(port))
}

/// Pick the active bearer token from config, or generate an opaque fallback.
pub fn choose_token(
    config_token: Option<&str>,
    generate: impl FnOnce() -> Result<String, String>,
) -> Result<(String, TokenSource), String> {
    match config_token.map(str::trim).filter(|s| !s.is_empty()) {
        Some(token) => Ok((token.to_string(), TokenSource::Config)),
        None => generate().map(|token| (token, TokenSource::Generated)),
    }
}

/// Look up the configured token using the documented precedence
/// (`TEXURE_TOKEN` env → profile → top-level), independent of `url`.
pub fn resolve_token_from_config(
    config: &TexureConfig,
    profile_override: Option<&str>,
) -> Result<Option<String>, String> {
    let get_env = |key: &str| std::env::var(key).ok();

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
        // An explicitly selected profile must exist; silently falling back to
        // the top-level token would hide a typo in --profile / TEXURE_PROFILE.
        Some(
            config
                .profiles
                .get(name)
                .ok_or_else(|| format!("profile '{name}' not found in config"))?,
        )
    } else {
        config
            .active_profile
            .as_deref()
            .and_then(|name| config.profiles.get(name))
    };

    Ok(get_env("TEXURE_TOKEN")
        .filter(|s| !s.trim().is_empty())
        .or_else(|| {
            profile
                .and_then(|p| p.token.clone())
                .filter(|s| !s.trim().is_empty())
        })
        .or_else(|| config.token.clone().filter(|s| !s.trim().is_empty()))
        .map(|s| s.trim().to_string()))
}

/// Generate a random, opaque, URL-safe pairing token backed by the OS CSPRNG.
///
/// A bearer pairing secret must be unguessable, so draw 32 bytes straight from
/// `getrandom` instead of relying on an undocumented `RandomState` seed. An
/// unavailable CSPRNG is an error — never silently downgrade to a weak token.
pub fn generate_pairing_token() -> Result<String, String> {
    use base64::Engine;

    let mut entropy = [0u8; 32];
    getrandom::fill(&mut entropy).map_err(|error| format!("OS CSPRNG unavailable: {error}"))?;
    Ok(base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(entropy))
}

/// Detect the LAN IP of this machine (UDP route probe), falling back to v4 loopback.
pub fn detect_lan_ip() -> IpAddr {
    use std::net::Ipv4Addr;
    use std::net::UdpSocket;

    let fallback = || {
        tracing::warn!("LAN IP detection failed; falling back to 127.0.0.1 — pass --tunnel-url for remote pairing");
        IpAddr::V4(Ipv4Addr::LOCALHOST)
    };
    match UdpSocket::bind("0.0.0.0:0") {
        Ok(socket) => match socket.connect("8.8.8.8:80") {
            Ok(()) => socket
                .local_addr()
                .map_or_else(|_| fallback(), |addr| addr.ip()),
            Err(_) => fallback(),
        },
        Err(_) => fallback(),
    }
}

/// Render a payload as an ASCII QR (`#` / space / newline, with a quiet zone).
pub fn render_ascii_qr(payload: &str) -> Result<String, String> {
    let code = qrcode::QrCode::new(payload)
        .map_err(|error| format!("failed to encode pairing payload as QR: {error}"))?;
    Ok(code
        .render::<char>()
        .quiet_zone(true)
        .module_dimensions(2, 1)
        .dark_color('#')
        .light_color(' ')
        .build())
}

/// Entry point for the `pair` subcommand: resolve endpoint + token, print QR
/// and connection string to stdout.
pub fn run_pair(
    config_path: Option<&Path>,
    profile: Option<&str>,
    tunnel_url: Option<&str>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let config = crate::config::read_config(config_path);
    let (token, source) = choose_token(
        resolve_token_from_config(&config, profile)?.as_deref(),
        generate_pairing_token,
    )?;

    let (host, port, path) = match tunnel_url {
        Some(raw) => {
            let override_endpoint = normalize_tunnel_url(raw)?;
            (
                override_endpoint.host,
                override_endpoint.port,
                override_endpoint.path,
            )
        }
        None => (
            detect_lan_ip().to_string(),
            DEFAULT_TUNNEL_PORT,
            DEFAULT_TUNNEL_PATH.to_string(),
        ),
    };

    let endpoint = PairingEndpoint {
        host,
        port,
        path,
        token,
    };
    let qr = render_ascii_qr(&endpoint.qr_payload_json())?;

    println!("{}", endpoint.connection_string());
    println!();
    println!("Scan this QR with the texure companion app:");
    println!("{qr}");

    match source {
        TokenSource::Config => eprintln!("Using bearer token from config."),
        TokenSource::Generated => {
            eprintln!("No configured token found; generated an ephemeral pairing token.")
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn endpoint() -> PairingEndpoint {
        PairingEndpoint {
            host: "192.168.1.42".to_string(),
            port: DEFAULT_TUNNEL_PORT,
            path: DEFAULT_TUNNEL_PATH.to_string(),
            token: "tok-123".to_string(),
        }
    }

    // (a) connection-string construction
    #[test]
    fn connection_string_uses_wss_host_port_path_and_token() {
        assert_eq!(
            endpoint().connection_string(),
            "wss://192.168.1.42:8766/tunnel?token=tok-123"
        );
    }

    #[test]
    fn qr_payload_json_matches_pairing_fixture_schema() {
        let json = endpoint().qr_payload_json();
        let v: serde_json::Value = serde_json::from_str(&json).expect("valid JSON payload");
        assert_eq!(v["scheme"].as_str(), Some("wss"));
        assert_eq!(v["host"].as_str(), Some("192.168.1.42"));
        assert_eq!(v["port"].as_u64(), Some(8766));
        assert_eq!(v["path"].as_str(), Some("/tunnel"));
        assert_eq!(v["token"].as_str(), Some("tok-123"));
        assert_eq!(v["token_present"].as_bool(), Some(true));
        assert_eq!(v["state"].as_str(), Some("UNPAIRED"));
        assert_eq!(v["app"].as_str(), Some("texure"));
    }

    // (b) --tunnel-url override normalization
    #[test]
    fn normalize_bare_domain_defaults_to_wss_443_and_tunnel_path() {
        assert_eq!(
            normalize_tunnel_url("tunnel.example.com").expect("normalize"),
            TunnelOverride {
                host: "tunnel.example.com".to_string(),
                port: DEFAULT_WSS_PORT,
                path: DEFAULT_TUNNEL_PATH.to_string(),
            }
        );
    }

    #[test]
    fn normalize_full_wss_url_preserves_explicit_port_and_path() {
        assert_eq!(
            normalize_tunnel_url("wss://abc.ngrok-free.app:8443/connect").expect("normalize"),
            TunnelOverride {
                host: "abc.ngrok-free.app".to_string(),
                port: 8443,
                path: "/connect".to_string(),
            }
        );
    }

    #[test]
    fn normalize_non_wss_scheme_is_forced_to_wss_and_path_normalized() {
        assert_eq!(
            normalize_tunnel_url("https://relay.example.org/tunnel/").expect("normalize"),
            TunnelOverride {
                host: "relay.example.org".to_string(),
                port: DEFAULT_WSS_PORT,
                path: "/tunnel/".to_string(),
            }
        );
    }

    #[test]
    fn normalize_rejects_empty_override() {
        assert!(normalize_tunnel_url("   ").is_err());
    }

    #[test]
    fn normalize_rejects_host_with_whitespace() {
        assert!(normalize_tunnel_url("bad host.example").is_err());
    }

    // (c) token reuse vs generated fallback
    #[test]
    fn choose_token_reuses_config_token_when_present() {
        let (token, source) = choose_token(Some("cfg-token"), || Ok("generated-token".to_string()))
            .expect("choose token");
        assert_eq!(token, "cfg-token");
        assert_eq!(source, TokenSource::Config);
    }

    #[test]
    fn choose_token_generates_fallback_when_config_token_absent_or_blank() {
        let (token, source) =
            choose_token(None, || Ok("generated-token".to_string())).expect("choose token");
        assert_eq!(token, "generated-token");
        assert_eq!(source, TokenSource::Generated);

        let (token, source) =
            choose_token(Some("   "), || Ok("generated-token".to_string())).expect("choose token");
        assert_eq!(token, "generated-token");
        assert_eq!(source, TokenSource::Generated);
    }

    #[test]
    fn resolve_token_from_config_prefers_profile_override() {
        use std::collections::HashMap;
        use texure_contract::config::ProfileConfig;

        let _env = crate::test_env_lock::lock();
        std::env::remove_var("TEXURE_TOKEN");
        std::env::remove_var("TEXURE_PROFILE");

        let mut profiles = HashMap::new();
        profiles.insert(
            "phone".to_string(),
            ProfileConfig {
                url: Some("http://phone:8765".to_string()),
                token: Some("phone-token".to_string()),
                timeout_secs: None,
            },
        );
        let config = TexureConfig {
            url: Some("http://default:8765".to_string()),
            token: Some("default-token".to_string()),
            active_profile: None,
            timeout_secs: None,
            profiles,
            ..Default::default()
        };

        let token = resolve_token_from_config(&config, Some("phone")).expect("profile exists");
        assert_eq!(token.as_deref(), Some("phone-token"));
    }

    #[test]
    fn connection_string_percent_encodes_configured_token() {
        let mut endpoint = endpoint();
        endpoint.token = "a+b&c".to_string();
        assert!(endpoint
            .connection_string()
            .ends_with("/tunnel?token=a%2Bb%26c"));
        // The JSON QR keeps the raw token so companion parsing matches.
        let payload: serde_json::Value =
            serde_json::from_str(&endpoint.qr_payload_json()).expect("valid JSON");
        assert_eq!(payload["token"], "a+b&c");
    }

    #[test]
    fn normalize_stops_authority_at_fragment_or_query() {
        let override_endpoint =
            normalize_tunnel_url("wss://relay.example#section").expect("normalize");
        assert_eq!(override_endpoint.host, "relay.example");
        assert_eq!(override_endpoint.path, DEFAULT_TUNNEL_PATH);

        let override_endpoint = normalize_tunnel_url("wss://relay.example?x=1").expect("normalize");
        assert_eq!(override_endpoint.host, "relay.example");
        assert_eq!(override_endpoint.path, DEFAULT_TUNNEL_PATH);
    }

    #[test]
    fn normalize_rejects_port_zero() {
        assert!(normalize_tunnel_url("wss://relay.example:0/tunnel").is_err());
    }

    #[test]
    fn resolve_token_from_config_rejects_unknown_selected_profile() {
        let _env = crate::test_env_lock::lock();
        std::env::remove_var("TEXURE_PROFILE");
        let config = TexureConfig {
            token: Some("top-level-token".to_string()),
            ..Default::default()
        };
        let err = resolve_token_from_config(&config, Some("missing")).expect_err("unknown profile");
        assert!(err.contains("missing"));
    }

    #[test]
    fn generated_token_is_opaque_url_safe_and_unique() {
        let a = generate_pairing_token().expect("CSPRNG available in test env");
        let b = generate_pairing_token().expect("CSPRNG available in test env");
        assert!(!a.is_empty());
        assert!(a
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'));
        assert_ne!(a, b, "successive generated tokens must differ");
    }

    // (d) ASCII QR renderer output shape
    #[test]
    fn ascii_qr_uses_only_blocks_spaces_and_newlines_with_quiet_zone() {
        let qr = render_ascii_qr("wss://192.168.1.42:8766/tunnel?token=tok-123").expect("qr");
        assert!(!qr.is_empty());
        assert!(qr.chars().all(|c| c == '#' || c == ' ' || c == '\n'));
        assert!(qr.contains('#'));

        let lines: Vec<&str> = qr.lines().collect();
        assert!(lines.len() > 4, "QR should have height beyond quiet zone");
        assert!(
            lines.first().expect("top row").chars().all(|c| c == ' '),
            "top quiet zone must be blank"
        );
        assert!(
            lines.last().expect("bottom row").chars().all(|c| c == ' '),
            "bottom quiet zone must be blank"
        );
        let mid = lines[lines.len() / 2];
        assert!(mid.starts_with(' '), "left quiet zone must be blank");
        assert!(mid.ends_with(' '), "right quiet zone must be blank");
    }
}
