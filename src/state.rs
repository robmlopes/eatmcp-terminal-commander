use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use serde::{Deserialize, Serialize};
use tokio::sync::{broadcast, RwLock};

use crate::environment::detector::detect_environment;
use crate::environment::fingerprint::EnvironmentFingerprint;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionRecord {
    pub id: String,
    pub timestamp: u64,
    pub source: String, // "stdio" or "web-dashboard"
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
}

impl AppState {
    pub fn new() -> Arc<Self> {
        let (event_tx, _) = broadcast::channel(256);
        Arc::new(Self {
            fingerprint_cache: RwLock::new(None),
            cache_ttl: Duration::from_secs(60),
            history: RwLock::new(Vec::new()),
            event_tx,
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
        {
            let mut hist = self.history.write().await;
            if hist.len() >= 100 {
                hist.remove(0);
            }
            hist.push(record.clone());
        }
        // Broadcast to live SSE subscribers (ignore error if no receivers)
        let _ = self.event_tx.send(record);
    }

    pub async fn get_history(&self) -> Vec<ExecutionRecord> {
        let hist = self.history.read().await;
        hist.clone()
    }
}

pub fn now_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
