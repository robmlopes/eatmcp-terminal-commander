use std::sync::Arc;
use std::time::Instant;
use sysinfo::System;

use super::protocol::{CallToolResult, ToolDefinition};
use crate::environment::capabilities::get_capabilities;
use crate::execution::process::run_command;
use crate::recovery::strategies::diagnose_failure;
use crate::safety::classifier::classify_command;
use crate::safety::policy::{PolicyAction, RiskLevel};
use crate::state::{now_timestamp, AppState, ExecutionRecord};
use crate::validation::checks::{
    validate_filesystem_path, validate_service_status, validate_tcp_port, ValidationStatus,
};

pub fn list_tools() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition {
            name: "environment.inspect".to_string(),
            description: "Inspect current host environment and return detailed fingerprint".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {},
                "additionalProperties": false
            }),
        },
        ToolDefinition {
            name: "environment.capabilities".to_string(),
            description: "List all capabilities supported by this environment and their concrete OS implementations".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {},
                "additionalProperties": false
            }),
        },
        ToolDefinition {
            name: "command.validate".to_string(),
            description: "Validate a command and its arguments against safety policies without executing it".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "command": { "type": "string", "description": "Program name or command" },
                    "args": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Command arguments"
                    }
                },
                "required": ["command"]
            }),
        },
        ToolDefinition {
            name: "command.execute".to_string(),
            description: "Execute a command on the host terminal with safety validation, timeout, and output capture".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "command": { "type": "string", "description": "Program executable name" },
                    "args": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Arguments to pass directly to executable"
                    },
                    "working_dir": { "type": "string", "description": "Optional working directory" },
                    "timeout_ms": { "type": "number", "description": "Timeout in milliseconds (default: 30000)" },
                    "use_shell": { "type": "boolean", "description": "Whether to run inside system shell (default: false)" }
                },
                "required": ["command"]
            }),
        },
        ToolDefinition {
            name: "service.status".to_string(),
            description: "Check the current state of a system service".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "name": { "type": "string", "description": "Service name" }
                },
                "required": ["name"]
            }),
        },
        ToolDefinition {
            name: "service.restart".to_string(),
            description: "Restart a system service and validate that it is active".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "name": { "type": "string", "description": "Service name" }
                },
                "required": ["name"]
            }),
        },
        ToolDefinition {
            name: "package.install".to_string(),
            description: "Install a software package using the detected environment package manager".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "package": { "type": "string", "description": "Package identifier to install" }
                },
                "required": ["package"]
            }),
        },
        ToolDefinition {
            name: "process.list".to_string(),
            description: "List currently running processes with PID, name, and memory usage".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "limit": { "type": "number", "description": "Maximum processes to return (default: 30)" }
                }
            }),
        },
        ToolDefinition {
            name: "network.port_check".to_string(),
            description: "Verify whether a TCP port is open and listening using native socket probe".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "host": { "type": "string", "description": "Host/IP to probe (default: 127.0.0.1)" },
                    "port": { "type": "number", "description": "TCP port number" },
                    "timeout_ms": { "type": "number", "description": "Probe timeout in ms (default: 2000)" }
                },
                "required": ["port"]
            }),
        },
        ToolDefinition {
            name: "filesystem.exists".to_string(),
            description: "Verify existence and metadata of a file or directory path".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "File or directory path" }
                },
                "required": ["path"]
            }),
        },
    ]
}

pub async fn call_tool(
    state: &Arc<AppState>,
    tool_name: &str,
    arguments: serde_json::Value,
) -> CallToolResult {
    let start = Instant::now();
    let fp = state.get_fingerprint().await;

    match tool_name {
        "environment.inspect" => {
            let res = CallToolResult::success_json(&fp).unwrap();
            record_call(state, tool_name, arguments, RiskLevel::Low, PolicyAction::Allow, Some(0), &res.content[0].text, "", start.elapsed().as_millis() as u64, ValidationStatus::Success, false, None, None, None).await;
            res
        }
        "environment.capabilities" => {
            let caps = get_capabilities(&fp);
            let res = CallToolResult::success_json(&caps).unwrap();
            record_call(state, tool_name, arguments, RiskLevel::Low, PolicyAction::Allow, Some(0), &res.content[0].text, "", start.elapsed().as_millis() as u64, ValidationStatus::Success, false, None, None, None).await;
            res
        }
        "command.validate" => {
            let cmd = arguments.get("command").and_then(|v| v.as_str()).unwrap_or("");
            let args: Vec<String> = arguments.get("args")
                .and_then(|v| v.as_array())
                .map(|arr| arr.iter().filter_map(|x| x.as_str().map(|s| s.to_string())).collect())
                .unwrap_or_default();

            let assessment = classify_command(cmd, &args);
            let payload = serde_json::json!({
                "command": cmd,
                "args": args,
                "risk_level": assessment.risk_level.to_string(),
                "action": assessment.action.to_string(),
                "reason": assessment.reason,
            });
            let res = CallToolResult::success_json(&payload).unwrap();
            record_call(state, tool_name, arguments, assessment.risk_level, assessment.action, Some(0), &res.content[0].text, "", start.elapsed().as_millis() as u64, ValidationStatus::Success, false, None, None, None).await;
            res
        }
        "command.execute" => {
            let cmd = arguments.get("command").and_then(|v| v.as_str()).unwrap_or("");
            let args: Vec<String> = arguments.get("args")
                .and_then(|v| v.as_array())
                .map(|arr| arr.iter().filter_map(|x| x.as_str().map(|s| s.to_string())).collect())
                .unwrap_or_default();
            let working_dir = arguments.get("working_dir").and_then(|v| v.as_str());
            let timeout_ms = arguments.get("timeout_ms").and_then(|v| v.as_u64()).unwrap_or(30000);
            let use_shell = arguments.get("use_shell").and_then(|v| v.as_bool()).unwrap_or(false);

            let assessment = classify_command(cmd, &args);
            if assessment.action == PolicyAction::Deny {
                let err_text = format!("SECURITY DENIAL: Command '{}' blocked by safety policy. Reason: {}", cmd, assessment.reason);
                record_call(state, tool_name, arguments, assessment.risk_level, assessment.action, Some(1), "", &err_text, start.elapsed().as_millis() as u64, ValidationStatus::Failed, true, Some("SECURITY_DENIED".to_string()), Some(assessment.reason), Some("Command violates safety policy and cannot be executed".to_string())).await;
                return CallToolResult::error(err_text);
            }

            match run_command(cmd, &args, working_dir, timeout_ms, use_shell, &fp.shell).await {
                Ok(proc_out) => {
                    let is_err = proc_out.exit_code != Some(0) || proc_out.timed_out;
                    let (val_status, err_class, cause, next_action) = if is_err {
                        let diag = diagnose_failure(cmd, proc_out.exit_code, &proc_out.stderr, proc_out.timed_out, &fp.platform);
                        (ValidationStatus::Failed, Some(diag.error_class.to_string()), Some(diag.likely_cause), Some(diag.suggested_next_action))
                    } else {
                        (ValidationStatus::Success, None, None, None)
                    };

                    let payload = serde_json::json!({
                        "exit_code": proc_out.exit_code,
                        "stdout": proc_out.stdout,
                        "stderr": proc_out.stderr,
                        "duration_ms": proc_out.duration_ms,
                        "timed_out": proc_out.timed_out,
                        "error_class": err_class,
                        "likely_cause": cause,
                        "suggested_next_action": next_action,
                    });
                    let res = if is_err {
                        CallToolResult::error(serde_json::to_string_pretty(&payload).unwrap())
                    } else {
                        CallToolResult::success(serde_json::to_string_pretty(&payload).unwrap())
                    };

                    record_call(state, tool_name, arguments, assessment.risk_level, assessment.action, proc_out.exit_code, &proc_out.stdout, &proc_out.stderr, proc_out.duration_ms, val_status, is_err, err_class, cause, next_action).await;
                    res
                }
                Err(err) => {
                    let err_text = format!("Failed to spawn process '{}': {}", cmd, err);
                    record_call(state, tool_name, arguments, assessment.risk_level, assessment.action, Some(1), "", &err_text, start.elapsed().as_millis() as u64, ValidationStatus::Failed, true, Some("SPAWN_ERROR".to_string()), Some(err.to_string()), Some("Ensure the executable exists and is accessible".to_string())).await;
                    CallToolResult::error(err_text)
                }
            }
        }
        "service.status" => {
            let svc_name = arguments.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let val = validate_service_status(svc_name, &fp.platform).await;
            let payload = serde_json::json!({
                "service": svc_name,
                "status": val.status.to_string(),
                "message": val.message,
                "details": val.details,
            });
            let res = CallToolResult::success_json(&payload).unwrap();
            record_call(state, tool_name, arguments, RiskLevel::Low, PolicyAction::Allow, Some(0), &res.content[0].text, "", start.elapsed().as_millis() as u64, val.status, false, None, None, None).await;
            res
        }
        "service.restart" => {
            let svc_name = arguments.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let (cmd, args) = if fp.platform == "windows" {
                ("sc.exe", vec!["stop".to_string(), svc_name.to_string()])
            } else {
                ("systemctl", vec!["restart".to_string(), svc_name.to_string()])
            };

            let proc_res = run_command(cmd, &args, None, 15000, false, &fp.shell).await;
            if fp.platform == "windows" {
                tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                let _ = run_command("sc.exe", &["start".to_string(), svc_name.to_string()], None, 15000, false, &fp.shell).await;
            }

            let val = validate_service_status(svc_name, &fp.platform).await;
            let duration_ms = start.elapsed().as_millis() as u64;
            let is_err = val.status == ValidationStatus::Failed;

            let payload = serde_json::json!({
                "service": svc_name,
                "action": "restart",
                "validation_status": val.status.to_string(),
                "validation_message": val.message,
                "details": val.details,
            });

            let res = if is_err {
                CallToolResult::error(serde_json::to_string_pretty(&payload).unwrap())
            } else {
                CallToolResult::success(serde_json::to_string_pretty(&payload).unwrap())
            };

            let (exit_code, stdout, stderr) = match proc_res {
                Ok(po) => (po.exit_code, po.stdout, po.stderr),
                Err(e) => (Some(1), String::new(), e.to_string()),
            };

            record_call(state, tool_name, arguments, RiskLevel::Medium, PolicyAction::Allow, exit_code, &stdout, &stderr, duration_ms, val.status, is_err, None, None, None).await;
            res
        }
        "package.install" => {
            let pkg = arguments.get("package").and_then(|v| v.as_str()).unwrap_or("");
            let (cmd, args) = match fp.package_manager.as_str() {
                "winget" => ("winget.exe", vec!["install".to_string(), "--id".to_string(), pkg.to_string(), "--silent".to_string(), "--accept-source-agreements".to_string(), "--accept-package-agreements".to_string()]),
                "choco" => ("choco.exe", vec!["install".to_string(), pkg.to_string(), "-y".to_string()]),
                "apt" => ("apt", vec!["install".to_string(), "-y".to_string(), pkg.to_string()]),
                "pkg" => ("pkg", vec!["install".to_string(), "-y".to_string(), pkg.to_string()]),
                _ => ("", vec![]),
            };

            if cmd.is_empty() {
                let err_text = format!("No supported package manager found on platform '{}'", fp.platform);
                record_call(state, tool_name, arguments, RiskLevel::Medium, PolicyAction::Allow, Some(1), "", &err_text, start.elapsed().as_millis() as u64, ValidationStatus::Failed, true, Some("NO_PM".to_string()), Some(err_text.clone()), None).await;
                return CallToolResult::error(err_text);
            }

            match run_command(cmd, &args, None, 120_000, false, &fp.shell).await {
                Ok(proc_out) => {
                    let is_err = proc_out.exit_code != Some(0);
                    let payload = serde_json::json!({
                        "package": pkg,
                        "package_manager": fp.package_manager,
                        "exit_code": proc_out.exit_code,
                        "stdout": proc_out.stdout,
                        "stderr": proc_out.stderr,
                        "duration_ms": proc_out.duration_ms,
                    });
                    let res = if is_err {
                        CallToolResult::error(serde_json::to_string_pretty(&payload).unwrap())
                    } else {
                        CallToolResult::success(serde_json::to_string_pretty(&payload).unwrap())
                    };
                    record_call(state, tool_name, arguments, RiskLevel::Medium, PolicyAction::Allow, proc_out.exit_code, &proc_out.stdout, &proc_out.stderr, proc_out.duration_ms, if is_err { ValidationStatus::Failed } else { ValidationStatus::Success }, is_err, None, None, None).await;
                    res
                }
                Err(e) => {
                    let err_text = format!("Failed to execute package installer: {}", e);
                    record_call(state, tool_name, arguments, RiskLevel::Medium, PolicyAction::Allow, Some(1), "", &err_text, start.elapsed().as_millis() as u64, ValidationStatus::Failed, true, None, None, None).await;
                    CallToolResult::error(err_text)
                }
            }
        }
        "process.list" => {
            let limit = arguments.get("limit").and_then(|v| v.as_u64()).unwrap_or(30) as usize;
            let mut sys = System::new_all();
            sys.refresh_all();

            let mut proc_list: Vec<serde_json::Value> = sys.processes().iter().map(|(pid, proc)| {
                serde_json::json!({
                    "pid": pid.as_u32(),
                    "name": proc.name().to_string_lossy(),
                    "memory_mb": proc.memory() / (1024 * 1024),
                    "cpu_usage": proc.cpu_usage(),
                })
            }).collect();

            proc_list.sort_by(|a, b| {
                b.get("memory_mb").and_then(|v| v.as_u64()).cmp(&a.get("memory_mb").and_then(|v| v.as_u64()))
            });
            proc_list.truncate(limit);

            let payload = serde_json::json!({
                "count": proc_list.len(),
                "processes": proc_list,
            });
            let res = CallToolResult::success_json(&payload).unwrap();
            record_call(state, tool_name, arguments, RiskLevel::Low, PolicyAction::Allow, Some(0), &res.content[0].text, "", start.elapsed().as_millis() as u64, ValidationStatus::Success, false, None, None, None).await;
            res
        }
        "network.port_check" => {
            let host = arguments.get("host").and_then(|v| v.as_str()).unwrap_or("127.0.0.1");
            let port = arguments.get("port").and_then(|v| v.as_u64()).unwrap_or(0) as u16;
            let timeout_ms = arguments.get("timeout_ms").and_then(|v| v.as_u64()).unwrap_or(2000);

            let val = validate_tcp_port(host, port, timeout_ms).await;
            let payload = serde_json::json!({
                "host": host,
                "port": port,
                "status": val.status.to_string(),
                "message": val.message,
                "details": val.details,
            });
            let is_err = val.status == ValidationStatus::Failed;
            let res = CallToolResult::success_json(&payload).unwrap();
            record_call(state, tool_name, arguments, RiskLevel::Low, PolicyAction::Allow, Some(0), &res.content[0].text, "", start.elapsed().as_millis() as u64, val.status, is_err, None, None, None).await;
            res
        }
        "filesystem.exists" => {
            let path_str = arguments.get("path").and_then(|v| v.as_str()).unwrap_or("");
            let val = validate_filesystem_path(path_str).await;
            let payload = serde_json::json!({
                "path": path_str,
                "status": val.status.to_string(),
                "message": val.message,
                "details": val.details,
            });
            let is_err = val.status == ValidationStatus::Failed;
            let res = CallToolResult::success_json(&payload).unwrap();
            record_call(state, tool_name, arguments, RiskLevel::Low, PolicyAction::Allow, Some(0), &res.content[0].text, "", start.elapsed().as_millis() as u64, val.status, is_err, None, None, None).await;
            res
        }
        _ => CallToolResult::error(format!("Unknown tool '{}'", tool_name)),
    }
}

async fn record_call(
    state: &Arc<AppState>,
    tool_name: &str,
    arguments: serde_json::Value,
    risk: RiskLevel,
    action: PolicyAction,
    exit_code: Option<i32>,
    stdout: &str,
    stderr: &str,
    duration_ms: u64,
    val_status: ValidationStatus,
    is_error: bool,
    error_class: Option<String>,
    likely_cause: Option<String>,
    suggested_next_action: Option<String>,
) {
    let rec = ExecutionRecord {
        id: format!("rec-{}", now_timestamp()),
        timestamp: now_timestamp(),
        source: "stdio".to_string(),
        tool_name: tool_name.to_string(),
        arguments,
        risk_level: risk.to_string(),
        policy_action: action.to_string(),
        exit_code,
        stdout: stdout.to_string(),
        stderr: stderr.to_string(),
        duration_ms,
        validation_status: val_status.to_string(),
        is_error,
        error_class,
        likely_cause,
        suggested_next_action,
    };
    state.record(rec).await;
}
