use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tracing::{debug, error, info, warn};

use super::protocol::{JsonRpcRequest, JsonRpcResponse, RequestId};
use crate::state::AppState;

pub async fn run_stdio_server(state: Arc<AppState>) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let stdin = tokio::io::stdin();
    let mut stdout = tokio::io::stdout();
    let mut reader = BufReader::new(stdin).lines();

    info!("EATMCP stdio transport initialized, listening on stdin");

    while let Some(line) = reader.next_line().await? {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        debug!(raw_message = trimmed, "Received stdio message");

        let parsed: serde_json::Value = match serde_json::from_str(trimmed) {
            Ok(v) => v,
            Err(err) => {
                error!(error = %err, "Failed to parse JSON-RPC payload");
                let err_resp = JsonRpcResponse::error(
                    RequestId::Str("null".to_string()),
                    -32700,
                    format!("Parse error: {}", err),
                    None,
                );
                send_response(&mut stdout, &err_resp).await?;
                continue;
            }
        };

        // Check if it's a notification (no "id" field)
        let has_id = parsed.get("id").is_some();
        if !has_id {
            if let Ok(method) = serde_json::from_value::<String>(parsed.get("method").cloned().unwrap_or_default()) {
                debug!(method = %method, "Received notification");
                // Standard MCP notifications like "notifications/initialized"
            }
            continue;
        }

        // It is a request
        let request: JsonRpcRequest = match serde_json::from_value(parsed) {
            Ok(req) => req,
            Err(err) => {
                warn!(error = %err, "Malformed request structure");
                let err_resp = JsonRpcResponse::error(
                    RequestId::Str("null".to_string()),
                    -32600,
                    "Invalid Request",
                    None,
                );
                send_response(&mut stdout, &err_resp).await?;
                continue;
            }
        };

        let resp = handle_request(&state, request).await;
        send_response(&mut stdout, &resp).await?;
    }

    info!("EATMCP stdio stream closed, terminating");
    Ok(())
}

async fn handle_request(state: &Arc<AppState>, req: JsonRpcRequest) -> JsonRpcResponse {
    let id = req.id.clone();
    match req.method.as_str() {
        "initialize" => {
            let result = serde_json::json!({
                "protocolVersion": "2024-11-05",
                "capabilities": {
                    "tools": {
                        "listChanged": true
                    }
                },
                "serverInfo": {
                    "name": "eatmcp",
                    "version": env!("CARGO_PKG_VERSION")
                }
            });
            JsonRpcResponse::success(id, result)
        }
        "ping" => JsonRpcResponse::success(id, serde_json::json!({})),
        "tools/list" => {
            let tools = super::tools::list_tools();
            JsonRpcResponse::success(id, serde_json::json!({ "tools": tools }))
        }
        "tools/call" => {
            let params = req.params.unwrap_or(serde_json::Value::Null);
            let tool_name = match params.get("name").and_then(|v| v.as_str()) {
                Some(n) => n,
                None => {
                    return JsonRpcResponse::error(id, -32602, "Missing 'name' in tools/call parameters", None);
                }
            };
            let arguments = params.get("arguments").cloned().unwrap_or(serde_json::json!({}));

            let result = super::tools::call_tool(state, tool_name, arguments).await;
            match serde_json::to_value(result) {
                Ok(val) => JsonRpcResponse::success(id, val),
                Err(err) => JsonRpcResponse::error(id, -32603, format!("Internal error: {}", err), None),
            }
        }
        _ => JsonRpcResponse::error(
            id,
            -32601,
            format!("Method not found: {}", req.method),
            None,
        ),
    }
}

async fn send_response<W: AsyncWriteExt + Unpin>(
    writer: &mut W,
    resp: &JsonRpcResponse,
) -> Result<(), std::io::Error> {
    let json_bytes = serde_json::to_vec(resp).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
    writer.write_all(&json_bytes).await?;
    writer.write_all(b"\n").await?;
    writer.flush().await?;
    Ok(())
}
