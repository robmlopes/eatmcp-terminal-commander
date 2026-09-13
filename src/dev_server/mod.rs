pub mod routes;
pub mod sse;

use std::sync::Arc;
use tracing::info;

use crate::state::AppState;

pub async fn start_dev_server(state: Arc<AppState>, host: &str, port: u16) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let router = routes::create_router(Arc::clone(&state));
    let addr = format!("{}:{}", host, port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    state.is_server.store(true, std::sync::atomic::Ordering::SeqCst);
    info!("EATMCP Dev Dashboard active at http://{}", addr);
    axum::serve(listener, router).await?;
    Ok(())
}
