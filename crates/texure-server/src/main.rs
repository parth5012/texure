pub mod bridge_client;
pub mod config;
pub mod mdns;
pub mod server;
pub mod wait_for;
pub mod ws_supervisor;

use clap::Parser;
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
    #[arg(long)]
    config: Option<PathBuf>,

    /// Optional device profile name to activate (overrides active_profile / TEXURE_PROFILE)
    #[arg(long)]
    profile: Option<String>,
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
    let server = McpServer::new(args.config, args.profile);

    server.run_stdio().await
}
