use std::sync::Arc;
use tracing::{error, info, warn};
use tracing_subscriber::EnvFilter;

use eatmcp::dev_server;
use eatmcp::mcp;
use eatmcp::state::AppState;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Critical: tracing MUST write to stderr so stdout is purely JSON-RPC
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(filter)
        .init();

    info!("Starting EATMCP (Environment-Aware Terminal MCP) v{}", env!("CARGO_PKG_VERSION"));

    let args: Vec<String> = std::env::args().collect();
    let mut dev_port: Option<u16> = std::env::var("EATMCP_DEV_PORT")
        .ok()
        .and_then(|p| p.parse().ok());
    let mut web_only = false;
    let mut no_dev = false;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--dev-port" => {
                if i + 1 < args.len() {
                    dev_port = args[i + 1].parse().ok();
                    i += 1;
                }
            }
            "--web-only" | "--ui-only" => {
                web_only = true;
            }
            "--no-dev" => {
                no_dev = true;
            }
            "--help" | "-h" => {
                eprintln!("EATMCP - Environment-Aware Terminal MCP Server");
                eprintln!("Usage: eatmcp [OPTIONS]");
                eprintln!();
                eprintln!("Options:");
                eprintln!("  --dev-port <PORT>   Start embedded Dev Web Dashboard on port (default: 3001)");
                eprintln!("  --web-only          Run only the Dev Web Dashboard (no stdio listener)");
                eprintln!("  --no-dev            Disable the embedded Dev Web Dashboard (stdio only)");
                eprintln!("  --help, -h          Print help information");
                return Ok(());
            }
            _ => {}
        }
        i += 1;
    }

    let state = AppState::new();

    // Default dev port to 3001 unless --no-dev
    if !no_dev && dev_port.is_none() {
        dev_port = Some(3001);
    }

    // Start Dev Web Dashboard if configured
    if let Some(port) = dev_port {
        let dev_state = Arc::clone(&state);
        tokio::spawn(async move {
            if let Err(e) = dev_server::start_dev_server(dev_state, port).await {
                warn!("Dev Web Dashboard could not start on port {}: {}", port, e);
            }
        });
    }

    if web_only {
        info!("Running in web-only mode. Press Ctrl+C to terminate.");
        tokio::signal::ctrl_c().await?;
        return Ok(());
    }

    // Run Stdio MCP Transport
    if let Err(e) = mcp::transport::run_stdio_server(state).await {
        error!("Fatal error in MCP stdio transport: {}", e);
        return Err(e);
    }

    Ok(())
}
