mod communication;
mod connection;
mod engine;
mod error;
mod params;
mod query;

use clap::Parser;
use tracing::info;
use tracing_subscriber::{EnvFilter, fmt, prelude::*};

#[derive(Parser)]
#[command(name = "kaiwadb-tunnel")]
#[command(about = "KaiwaDB Tunnel WebSocket client")]
#[command(version)]
struct Args {
    /// WebSocket URL to connect to
    #[arg(short, long, default_value = "wss://api.kaiwadb.com/tunnel/connector")]
    uri: String,

    /// Authentication token
    #[arg(short, long, env = "KAIWADB_TUNNEL_TOKEN")]
    token: String,
}

#[tokio::main]
async fn main() -> Result<(), error::TunnelError> {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let registry = tracing_subscriber::registry().with(filter);

    if std::io::IsTerminal::is_terminal(&std::io::stdout()) {
        registry.with(fmt::layer().pretty()).init();
    } else {
        registry.with(fmt::layer().json()).init();
    }

    let args = Args::parse();

    info!(uri = %args.uri, "starting kaiwadb tunnel");
    connection::run(args.uri, args.token).await
}
