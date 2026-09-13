use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::{broadcast, RwLock};

use crate::environment::detector::detect_environment;
use crate::environment::fingerprint::EnvironmentFingerprint;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionRecord {
    pub id: String,
    pub timestamp: u64,
    pub source: String, // "stdio" or "web-dashboard"
    #[serde(default)]
    pub project: String,
    #[serde(default)]
    pub pid: u32,
    pub tool_name: String,
    pub arguments: serde_json::Value,
    pub risk_level: String,
    pub policy_action: String,
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub duration_ms: u64,
    pub validation_status: String,
    pub is_error: bool,
    pub error_class: Option<String>,
    pub likely_cause: Option<String>,
    pub suggested_next_action: Option<String>,
}

pub struct AppState {
    fingerprint_cache: RwLock<Option<(EnvironmentFingerprint, Instant)>>,
    cache_ttl: Duration,
    history: RwLock<Vec<ExecutionRecord>>,
    pub event_tx: broadcast::Sender<ExecutionRecord>,
    pub is_server: AtomicBool,
    pub hub_host: String,
    pub hub_port: u16,
}

impl AppState {
    pub fn new(hub_host: String, hub_port: u16) -> Arc<Self> {
        let (event_tx, _) = broadcast::channel(256);
        Arc::new(Self {
            fingerprint_cache: RwLock::new(None),
            cache_ttl: Duration::from_secs(60),
            history: RwLock::new(Vec::new()),
            event_tx,
            is_server: AtomicBool::new(false),
            hub_host,
            hub_port,
        })
    }

    pub async fn get_fingerprint(&self) -> EnvironmentFingerprint {
        {
            let read_guard = self.fingerprint_cache.read().await;
            if let Some((fp, cached_at)) = &*read_guard {
                if cached_at.elapsed() < self.cache_ttl {
                    let mut cached_fp = fp.clone();
                    cached_fp.cache_age_ms = cached_at.elapsed().as_millis() as u64;
                    return cached_fp;
                }
            }
        }

        // Cache miss or expired: detect and store
        let fresh_fp = detect_environment().await;
        let mut write_guard = self.fingerprint_cache.write().await;
        *write_guard = Some((fresh_fp.clone(), Instant::now()));
        fresh_fp
    }

    pub async fn record(&self, record: ExecutionRecord) {
        self.record_internal(record, true).await;
    }

    pub async fn record_locally(&self, record: ExecutionRecord) {
        self.record_internal(record, false).await;
    }

    async fn record_internal(&self, record: ExecutionRecord, forward: bool) {
        {
            let mut hist = self.history.write().await;
            if hist.len() >= 200 {
                hist.remove(0);
            }
            hist.push(record.clone());
        }
        // Broadcast to local live SSE subscribers
        let _ = self.event_tx.send(record.clone());

        // If this instance is NOT the server, forward the record to the central dev dashboard
        if forward && !self.is_server.load(Ordering::SeqCst) {
            let host = self.hub_host.clone();
            let port = self.hub_port;
            let _ = tokio::time::timeout(Duration::from_millis(500), forward_to_hub(&host, port, &record)).await;
        }
    }

    pub async fn get_history(&self) -> Vec<ExecutionRecord> {
        let hist = self.history.read().await;
        hist.clone()
    }
}

async fn forward_to_hub(
    host: &str,
    port: u16,
    record: &ExecutionRecord,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let json_bytes = serde_json::to_vec(record)?;
    let addr = format!("{}:{}", host, port);
    let connect_timeout = Duration::from_millis(500);

    let mut stream = tokio::time::timeout(connect_timeout, tokio::net::TcpStream::connect(&addr)).await??;

    let header = format!(
        "POST /api/record HTTP/1.1\r\nHost: {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        addr,
        json_bytes.len()
    );

    tokio::time::timeout(connect_timeout, async {
        stream.write_all(header.as_bytes()).await?;
        stream.write_all(&json_bytes).await?;
        stream.flush().await?;
        let mut buf = [0u8; 64];
        let _ = stream.read(&mut buf).await;
        Ok::<(), std::io::Error>(())
    })
    .await??;

    Ok(())
}

pub fn now_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
