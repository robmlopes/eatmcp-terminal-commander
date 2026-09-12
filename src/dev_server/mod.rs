pub mod routes;
pub mod sse;

use std::sync::Arc;
use tracing::info;

use crate::state::AppState;

pub async fn start_dev_server(state: Arc<AppState>, port: u16) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let router = routes::create_router(state);
    let addr = format!("127.0.0.1:{}", port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    info!("EATMCP Dev Dashboard active at http://{}", addr);
    axum::serve(listener, router).await?;
    Ok(())
}
