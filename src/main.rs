use std::sync::Arc;
use tracing::{error, info};
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
    let mut dev_host: String = std::env::var("EATMCP_DEV_HOST")
        .unwrap_or_else(|_| "127.0.0.1".to_string());
    let mut dev_port: Option<u16> = std::env::var("EATMCP_DEV_PORT")
        .ok()
        .and_then(|p| p.parse().ok());
    let mut web_only = false;
    let mut no_dev = false;

    let mut show_configs_requested = false;
    let mut install_target: Option<String> = None;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--dev-port" | "--port" => {
                if i + 1 < args.len() {
                    dev_port = args[i + 1].parse().ok();
                    i += 1;
                }
            }
            "--dev-host" | "--host" => {
                if i + 1 < args.len() {
                    dev_host = args[i + 1].clone();
                    i += 1;
                }
            }
            "--web-only" | "--ui-only" => {
                web_only = true;
            }
            "--no-dev" => {
                no_dev = true;
            }
            "--show-configs" => {
                show_configs_requested = true;
            }
            "--install" => {
                let target = if i + 1 < args.len() && !args[i + 1].starts_with('-') {
                    i += 1;
                    args[i].clone()
                } else {
                    "all".to_string()
                };
                install_target = Some(target);
            }
            "--help" | "-h" => {
                eprintln!("EATMCP - Environment-Aware Terminal MCP Server");
                eprintln!("Usage: eatmcp [OPTIONS]");
                eprintln!();
                eprintln!("Options:");
                eprintln!("  --show-configs      Display MCP JSON configs for all major AI harnesses");
                eprintln!("  --install <target>  Auto-install config into harness (claude-desktop, cursor, windsurf, all)");
                eprintln!("  --host <HOST>       Bind address for the Dev Web Dashboard (default: 127.0.0.1)");
                eprintln!("  --port <PORT>       Port for the Dev Web Dashboard (default: 3001)");
                eprintln!("  --web-only          Run only the Dev Web Dashboard (no stdio listener)");
                eprintln!("  --no-dev            Disable the embedded Dev Web Dashboard (stdio only)");
                eprintln!("  --help, -h          Print help information");
                return Ok(());
            }
            _ => {}
        }
        i += 1;
    }

    let port = dev_port.unwrap_or(3001);

    if show_configs_requested {
        eatmcp::cli::installer::show_configs(&dev_host, port);
        return Ok(());
    }

    if let Some(target) = install_target {
        if let Err(e) = eatmcp::cli::installer::install_for(&target, &dev_host, port) {
            eprintln!("Error installing for '{}': {}", target, e);
        }
        return Ok(());
    }

    let state = AppState::new(dev_host.clone(), port);

    // Start Dev Web Dashboard if configured
    if !no_dev {
        let dev_state = Arc::clone(&state);
        let host = dev_host.clone();
        tokio::spawn(async move {
            if let Err(e) = dev_server::start_dev_server(Arc::clone(&dev_state), &host, port).await {
                info!("Dev Web Dashboard already active on {}:{} - this instance will relay events to it ({})", host, port, e);
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
