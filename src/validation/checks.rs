use std::path::Path;
use std::time::Duration;
use serde::{Deserialize, Serialize};
use tokio::net::TcpStream;
use tokio::time::timeout;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ValidationStatus {
    Success,
    PartialSuccess,
    Failed,
    Unknown,
}

impl std::fmt::Display for ValidationStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ValidationStatus::Success => write!(f, "SUCCESS"),
            ValidationStatus::PartialSuccess => write!(f, "PARTIAL_SUCCESS"),
            ValidationStatus::Failed => write!(f, "FAILED"),
            ValidationStatus::Unknown => write!(f, "UNKNOWN"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResult {
    pub status: ValidationStatus,
    pub message: String,
    pub details: serde_json::Value,
}

pub async fn validate_tcp_port(host: &str, port: u16, timeout_ms: u64) -> ValidationResult {
    let addr = format!("{}:{}", host, port);
    let dur = Duration::from_millis(if timeout_ms == 0 { 2000 } else { timeout_ms });

    match timeout(dur, TcpStream::connect(&addr)).await {
        Ok(Ok(_stream)) => ValidationResult {
            status: ValidationStatus::Success,
            message: format!("TCP port {}:{} is open and accepting connections", host, port),
            details: serde_json::json!({ "host": host, "port": port, "open": true }),
        },
        Ok(Err(e)) => ValidationResult {
            status: ValidationStatus::Failed,
            message: format!("TCP port {}:{} connection refused or unreachable ({})", host, port, e),
            details: serde_json::json!({ "host": host, "port": port, "open": false, "error": e.to_string() }),
        },
        Err(_) => ValidationResult {
            status: ValidationStatus::Failed,
            message: format!("TCP port {}:{} probe timed out after {}ms", host, port, dur.as_millis()),
            details: serde_json::json!({ "host": host, "port": port, "timed_out": true }),
        },
    }
}

pub async fn validate_filesystem_path(path_str: &str) -> ValidationResult {
    let p = Path::new(path_str);
    match tokio::fs::metadata(p).await {
        Ok(meta) => {
            let is_dir = meta.is_dir();
            let is_file = meta.is_file();
            let size = meta.len();
            ValidationResult {
                status: ValidationStatus::Success,
                message: format!("Path '{}' exists (is_dir: {}, is_file: {}, size: {} bytes)", path_str, is_dir, is_file, size),
                details: serde_json::json!({
                    "path": path_str,
                    "exists": true,
                    "is_directory": is_dir,
                    "is_file": is_file,
                    "size_bytes": size,
                }),
            }
        }
        Err(e) => ValidationResult {
            status: ValidationStatus::Failed,
            message: format!("Path '{}' does not exist or cannot be accessed ({})", path_str, e),
            details: serde_json::json!({
                "path": path_str,
                "exists": false,
                "error": e.to_string(),
            }),
        },
    }
}

pub async fn validate_service_status(service_name: &str, platform: &str) -> ValidationResult {
    if platform == "windows" {
        let out = tokio::process::Command::new("sc.exe")
            .args(&["query", service_name])
            .output()
            .await;

        match out {
            Ok(output) => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                if stdout.contains("RUNNING") {
                    ValidationResult {
                        status: ValidationStatus::Success,
                        message: format!("Service '{}' is RUNNING", service_name),
                        details: serde_json::json!({ "service": service_name, "state": "RUNNING" }),
                    }
                } else if stdout.contains("STOPPED") {
                    ValidationResult {
                        status: ValidationStatus::PartialSuccess,
                        message: format!("Service '{}' is STOPPED", service_name),
                        details: serde_json::json!({ "service": service_name, "state": "STOPPED" }),
                    }
                } else {
                    ValidationResult {
                        status: ValidationStatus::Failed,
                        message: format!("Service '{}' query returned non-running state", service_name),
                        details: serde_json::json!({ "service": service_name, "raw": stdout.to_string() }),
                    }
                }
            }
            Err(e) => ValidationResult {
                status: ValidationStatus::Unknown,
                message: format!("Failed to query service state via sc.exe: {}", e),
                details: serde_json::json!({ "error": e.to_string() }),
            },
        }
    } else {
        let out = tokio::process::Command::new("systemctl")
            .args(&["is-active", service_name])
            .output()
            .await;

        match out {
            Ok(output) => {
                let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if stdout == "active" {
                    ValidationResult {
                        status: ValidationStatus::Success,
                        message: format!("Service '{}' is active", service_name),
                        details: serde_json::json!({ "service": service_name, "state": stdout }),
                    }
                } else {
                    ValidationResult {
                        status: ValidationStatus::Failed,
                        message: format!("Service '{}' state is '{}'", service_name, stdout),
                        details: serde_json::json!({ "service": service_name, "state": stdout }),
                    }
                }
            }
            Err(e) => ValidationResult {
                status: ValidationStatus::Unknown,
                message: format!("Failed to query systemctl: {}", e),
                details: serde_json::json!({ "error": e.to_string() }),
            },
        }
    }
}
