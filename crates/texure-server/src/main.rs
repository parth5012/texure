pub mod bridge_client;
pub mod config;
pub mod mdns;
pub mod pair;
pub mod router;
pub mod server;
pub mod transport;
pub mod tunnel_server;
pub mod wait_for;
pub mod ws_supervisor;

use clap::{Parser, Subcommand};
use server::McpServer;
use std::path::PathBuf;
use tracing_subscriber::EnvFilter;

/// Serialize tests that mutate process environment variables; cargo runs
/// tests in parallel threads within one binary and env mutations race.
#[cfg(test)]
pub(crate) mod test_env_lock {
    use std::sync::{Mutex, MutexGuard, OnceLock};
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    pub fn lock() -> MutexGuard<'static, ()> {
        LOCK.get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap_or_else(|e| e.into_inner())
    }
}

#[derive(Parser, Debug)]
#[command(
    name = "texure-server",
    about = "Rust MCP stdio server for the texure Android bridge"
)]
struct Args {
    /// Optional path to custom configuration file
    #[arg(long, global = true)]
    config: Option<PathBuf>,

    /// Optional device profile name to activate (overrides active_profile / TEXURE_PROFILE)
    #[arg(long, global = true)]
    profile: Option<String>,

    /// Opt-in: bind the reverse WebSocket tunnel endpoint (/tunnel) for the remote companion
    #[arg(long, global = true)]
    enable_tunnel: bool,

    /// Port for the reverse tunnel listener; passing it implies --enable-tunnel (default 8766)
    #[arg(long, global = true)]
    tunnel_port: Option<u16>,

    /// Opt-in: probe the companion for STOPPED state and trigger shizuku_start_app before tool calls.
    /// Requires the companion to publish a `device_state` RPC (see DECISIONS.md); off by default.
    #[arg(long, global = true)]
    lifecycle_probe: bool,

    /// Optional subcommand; omit to run the stdio MCP server
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Print a scannable pairing QR and the raw wss:// connection string
    Pair {
        /// Tunnel override: a full wss://host[:port]/path or a bare domain
        #[arg(long)]
        tunnel_url: Option<String>,
    },
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Stderr-only logging to avoid corrupting standard I/O JSON-RPC transport on stdout
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("warn")),
        )
        .with_writer(std::io::stderr)
        .init();

    let args = Args::parse();
    let config = args.config;
    let profile = args.profile;
    let enable_tunnel = args.enable_tunnel;
    let tunnel_port = args.tunnel_port;
    let lifecycle_probe = args.lifecycle_probe;

    match args.command {
        Some(Command::Pair { tunnel_url }) => {
            pair::run_pair(config.as_deref(), profile.as_deref(), tunnel_url.as_deref())
        }
        None => {
            // Opt-in only. With neither flag, this stays the plain stdio MCP
            // server (the mcp-real blackbox depends on unchanged behavior).
            let _tunnel_server = if enable_tunnel || tunnel_port.is_some() {
                let file_config = crate::config::read_config(config.as_deref());
                let token = pair::resolve_token_from_config(&file_config, profile.as_deref())
                    .map_err(std::io::Error::other)?
                    .ok_or_else(|| {
                        std::io::Error::new(
                            std::io::ErrorKind::InvalidInput,
                            "--enable-tunnel requires a bearer token in config or TEXURE_TOKEN",
                        )
                    })?;
                let port = tunnel_port.unwrap_or(pair::DEFAULT_TUNNEL_PORT);
                let tunnel = tunnel_server::TunnelServer::bind(port, token).await?;
                eprintln!(
                    "Reverse tunnel listening on 0.0.0.0:{}{} (TLS terminated externally)",
                    tunnel.local_addr().port(),
                    pair::DEFAULT_TUNNEL_PATH
                );
                Some(tunnel)
            } else {
                None
            };

            let server = McpServer::new(config, profile);
            // Pass the tunnel handle (when opt-in) into the MCP server so it
            // can select tunnel-vs-direct routing per tool call.
            let mut server = match _tunnel_server.as_ref() {
                Some(tunnel) => server.with_tunnel(tunnel.handle()),
                None => server,
            };
            if lifecycle_probe {
                server = server.with_lifecycle_probe();
            }
            server.run_stdio().await
        }
    }
}
