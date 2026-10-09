use std::sync::Arc;
use std::time::Instant;
use sysinfo::System;

use super::protocol::{CallToolResult, ToolDefinition};
use crate::environment::capabilities::get_capabilities;
use crate::execution::process::run_command;
use crate::recovery::strategies::diagnose_failure;
use crate::safety::classifier::classify_command;
use crate::safety::policy::{PolicyAction, RiskLevel};
use crate::state::{now_timestamp, now_timestamp_nanos, AppState, ExecutionRecord};
use crate::validation::checks::{
    validate_filesystem_path, validate_service_status, validate_tcp_port, ValidationStatus,
};

pub fn list_tools() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition {
            name: "environment_inspect".to_string(),
            description: "MANDATORY ENVIRONMENT DISCOVERY: Call this BEFORE executing unfamiliar terminal commands to discover host OS, distribution, architecture, shell, init system, package manager, and privilege level.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {},
                "additionalProperties": false
            }),
        },
        ToolDefinition {
            name: "environment_capabilities".to_string(),
            description: "CAPABILITY REGISTRY: List all capabilities supported by this specific operating environment and their concrete command-line implementations.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {},
                "additionalProperties": false
            }),
        },
        ToolDefinition {
            name: "command_validate".to_string(),
            description: "PRE-EXECUTION SAFETY CHECK: Validate any terminal command and its arguments against system safety policies, classifying risk (LOW, MEDIUM, HIGH, CRITICAL) without running it.".to_string(),
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
            name: "command_execute".to_string(),
            description: "PRIMARY TERMINAL EXECUTION ENGINE: Execute commands across shells (bash, cmd, powershell, direct). Preserves exact terminal output and dialect rules. 'auto' selects based on command dialect.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "command": { "type": "string", "description": "Command line to execute, or program executable name" },
                    "args": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Optional arguments to pass to command (if not included in command string)"
                    },
                    "shell": {
                        "type": "string",
                        "enum": ["auto", "bash", "cmd", "powershell", "direct"],
                        "description": "Terminal environment: 'auto' (default), 'bash' (POSIX/Linux output via bash -c), 'cmd' (Windows CMD via cmd.exe /c), 'powershell' (PowerShell object/table output), 'direct' (spawn process directly without shell)"
                    },
                    "working_dir": { "type": "string", "description": "Optional working directory" },
                    "timeout_ms": { "type": "number", "description": "Timeout in milliseconds (default: 30000)" },
                    "use_shell": { "type": "boolean", "description": "Legacy alias for shell mode (default: false)" }
                },
                "required": ["command"]
            }),
        },
        ToolDefinition {
            name: "service_status".to_string(),
            description: "Check the current state of a system service using native service manager (sc.exe on Windows, systemctl on Linux).".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "name": { "type": "string", "description": "Service name" }
                },
                "required": ["name"]
            }),
        },
        ToolDefinition {
            name: "service_restart".to_string(),
            description: "Restart a system service and automatically validate active state post-condition.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "name": { "type": "string", "description": "Service name" }
                },
                "required": ["name"]
            }),
        },
        ToolDefinition {
            name: "package_install".to_string(),
            description: "Install a software package using the detected environment package manager (winget, apt, pkg, choco).".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "package": { "type": "string", "description": "Package identifier to install" }
                },
                "required": ["package"]
            }),
        },
        ToolDefinition {
            name: "process_list".to_string(),
            description: "List currently running processes with PID, name, memory, and CPU usage via native OS APIs.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "limit": { "type": "number", "description": "Maximum processes to return (default: 30)" }
                }
            }),
        },
        ToolDefinition {
            name: "network_port_check".to_string(),
            description: "Verify whether a TCP port is open and listening using native non-blocking socket probe.".to_string(),
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
            name: "filesystem_exists".to_string(),
            description: "Verify existence, type, and size of a file or directory path natively.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "File or directory path" }
                },
                "required": ["path"]
            }),
        },
        ToolDefinition {
            name: "file_read".to_string(),
            description: "OPTIMIZED FILE READER: Read file lines with 1-based offset and line limit. Formatted as dense 'line:content'. Automatically caps large outputs to protect context window.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Path to file" },
                    "offset": { "type": "number", "description": "1-based line number to start reading from (default: 1)" },
                    "limit": { "type": "number", "description": "Maximum number of lines to return (default: 2000)" },
                    "max_bytes": { "type": "number", "description": "Maximum byte length before truncation (default: 131072)" }
                },
                "required": ["path"]
            }),
        },
        ToolDefinition {
            name: "file_edit".to_string(),
            description: "ATOMIC FILE EDITOR: Exact string replacement in file with unified diff generation. Fails safely if match count != 1 (unless replace_all is true). Returns unified diff and structured diff metadata.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Path to file" },
                    "old_string": { "type": "string", "description": "Exact text to replace" },
                    "new_string": { "type": "string", "description": "Replacement text" },
                    "replace_all": { "type": "boolean", "description": "Replace all occurrences instead of requiring exactly 1 match (default: false)" }
                },
                "required": ["path", "old_string", "new_string"]
            }),
        },
        ToolDefinition {
            name: "file_write".to_string(),
            description: "DIRECT FILE WRITER: Create or overwrite file with buffer directly. Auto-creates parent directories. Atomic disk flush without shell escaping fragility.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Path to file" },
                    "content": { "type": "string", "description": "Complete text content to write" },
                    "create_dirs": { "type": "boolean", "description": "Create missing parent directories (default: true)" }
                },
                "required": ["path", "content"]
            }),
        },
        ToolDefinition {
            name: "file_search".to_string(),
            description: "UNIFIED FILE & CONTENT SEARCH (GLOB + GREP): Search file paths or regex patterns inside files. Respects .gitignore and excludes hidden directories. Output formatted as ultra-dense 'path:line:content'.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Directory to search in (default: '.')" },
                    "pattern": { "type": "string", "description": "Regex pattern or substring to search inside files (if empty, matches file paths via glob)" },
                    "glob": { "type": "string", "description": "Optional glob pattern to filter files (e.g. '*.rs', '**/*.ts')" },
                    "max_results": { "type": "number", "description": "Maximum results to return (default: 250)" },
                    "case_sensitive": { "type": "boolean", "description": "Case-sensitive regex search (default: true)" }
                }
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
    let normalized = tool_name.replace('.', "_");

    match normalized.as_str() {
        "environment_inspect" => {
            let res = CallToolResult::success_json(&fp).unwrap();
            record_call(state, tool_name, arguments, RiskLevel::Low, PolicyAction::Allow, Some(0), &res.content[0].text, "", start.elapsed().as_millis() as u64, ValidationStatus::Success, false, None, None, None).await;
            res
        }
        "environment_capabilities" => {
            let caps = get_capabilities(&fp);
            let res = CallToolResult::success_json(&caps).unwrap();
            record_call(state, tool_name, arguments, RiskLevel::Low, PolicyAction::Allow, Some(0), &res.content[0].text, "", start.elapsed().as_millis() as u64, ValidationStatus::Success, false, None, None, None).await;
            res
        }
        "command_validate" => {
            let cmd = arguments.get("command").and_then(|v| v.as_str()).unwrap_or("");
            let args: Vec<String> = arguments.get("args")
                .and_then(|v| v.as_array())
                .map(|arr| arr.iter().filter_map(|x| x.as_str().map(|s| s.to_string())).collect())
                .unwrap_or_default();
            let requested_shell = arguments.get("shell")
                .and_then(|v| v.as_str())
                .unwrap_or("auto");

            let assessment = classify_command(cmd, &args);
            let resolution = crate::execution::resolver::resolve_command(cmd, requested_shell);
            let payload = serde_json::json!({
                "command": cmd,
                "args": args,
                "dialect": resolution.dialect.to_string(),
                "risk_level": assessment.risk_level.to_string(),
                "action": assessment.action.to_string(),
                "reason": assessment.reason,
                "resolved_executable": resolution.resolved_executable,
                "resolution_source": resolution.resolution_source,
                "recommended_shell": resolution.recommended_shell,
                "shell_compatibility": resolution.shell_compatibility,
            });
            let res = CallToolResult::success_json(&payload).unwrap();
            record_call(state, tool_name, arguments, assessment.risk_level, assessment.action, Some(0), &res.content[0].text, "", start.elapsed().as_millis() as u64, ValidationStatus::Success, false, None, None, None).await;
            res
        }
        "command_execute" => {
            let cmd = arguments.get("command").and_then(|v| v.as_str()).unwrap_or("");
            let args: Vec<String> = arguments.get("args")
                .and_then(|v| v.as_array())
                .map(|arr| arr.iter().filter_map(|x| x.as_str().map(|s| s.to_string())).collect())
                .unwrap_or_default();
            let working_dir = arguments.get("working_dir").and_then(|v| v.as_str());
            let timeout_ms = arguments.get("timeout_ms").and_then(|v| v.as_u64()).unwrap_or(30000);
            let requested_shell = arguments.get("shell")
                .and_then(|v| v.as_str())
                .or_else(|| {
                    if arguments.get("use_shell").and_then(|v| v.as_bool()).unwrap_or(false) {
                        Some("auto")
                    } else {
                        None
                    }
                })
                .unwrap_or("auto");

            let assessment = classify_command(cmd, &args);
            let resolution = crate::execution::resolver::resolve_command(cmd, requested_shell);
            if assessment.action == PolicyAction::Deny {
                let err_text = format!("SECURITY DENIAL: Command '{}' blocked by safety policy. Reason: {}", cmd, assessment.reason);
                record_call(state, tool_name, arguments, assessment.risk_level, assessment.action, Some(1), "", &err_text, start.elapsed().as_millis() as u64, ValidationStatus::Failed, true, Some("SECURITY_DENIED".to_string()), Some(assessment.reason), Some("Command violates safety policy and cannot be executed".to_string())).await;
                return CallToolResult::error(err_text);
            }

            match run_command(cmd, &args, working_dir, timeout_ms, requested_shell).await {
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
                        "dialect": resolution.dialect.to_string(),
                        "resolution_source": resolution.resolution_source,
                        "executed_via": resolution.recommended_shell,
                        "shell_compatibility": resolution.shell_compatibility,
                        "error_class": err_class,
                        "likely_cause": cause,
                        "suggested_next_action": next_action,
                    });
                    let terminal_output = format_terminal_output(
                        &proc_out.stdout,
                        &proc_out.stderr,
                        proc_out.exit_code,
                        proc_out.timed_out,
                    );
                    let mut res = if is_err {
                        CallToolResult::error(terminal_output)
                    } else {
                        CallToolResult::success(terminal_output)
                    };
                    res.meta = Some(payload);

                    record_call(state, tool_name, arguments, assessment.risk_level, assessment.action, proc_out.exit_code, &proc_out.stdout, &proc_out.stderr, proc_out.duration_ms, val_status, is_err, err_class, cause, next_action).await;
                    res
                }
                Err(err) => {
                    let err_text = format!("Failed to spawn process '{}': {}", cmd, err);
                    let (cause, suggestion) = if err.kind() == std::io::ErrorKind::NotFound {
                        (
                            format!("Executable or script '{}' was not found in PATH or cannot be spawned directly.", cmd),
                            crate::recovery::strategies::suggest_missing_program(cmd, &fp.platform),
                        )
                    } else {
                        (err.to_string(), "Ensure the executable exists, permissions are granted, and working directory is valid.".to_string())
                    };

                    record_call(state, tool_name, arguments, assessment.risk_level, assessment.action, Some(1), "", &err_text, start.elapsed().as_millis() as u64, ValidationStatus::Failed, true, Some("SPAWN_ERROR".to_string()), Some(cause), Some(suggestion)).await;
                    CallToolResult::error(err_text)
                }
            }
        }
        "service_status" => {
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
        "service_restart" => {
            let svc_name = arguments.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let (cmd, args) = if fp.platform == "windows" {
                ("sc.exe", vec!["stop".to_string(), svc_name.to_string()])
            } else {
                ("systemctl", vec!["restart".to_string(), svc_name.to_string()])
            };

            let proc_res = run_command(cmd, &args, None, 15000, "auto").await;
            if fp.platform == "windows" {
                tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                let _ = run_command("sc.exe", &["start".to_string(), svc_name.to_string()], None, 15000, "auto").await;
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
        "package_install" => {
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

            match run_command(cmd, &args, None, 120_000, "auto").await {
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
        "process_list" => {
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
        "network_port_check" => {
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
        "filesystem_exists" => {
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
        "file_read" => {
            let path_str = arguments.get("path").and_then(|v| v.as_str()).unwrap_or("");
            let offset = arguments.get("offset").and_then(|v| v.as_u64()).map(|v| v as usize);
            let limit = arguments.get("limit").and_then(|v| v.as_u64()).map(|v| v as usize);
            let max_bytes = arguments.get("max_bytes").and_then(|v| v.as_u64()).map(|v| v as usize);

            let res = match crate::fs::read_file(path_str, offset, limit, max_bytes) {
                Ok(content) => CallToolResult::success(content),
                Err(err) => CallToolResult::error(err),
            };
            let is_err = res.is_error;
            let out_text = res.content.first().map(|c| c.text.as_str()).unwrap_or("");
            record_call(state, tool_name, arguments, RiskLevel::Low, PolicyAction::Allow, Some(if is_err { 1 } else { 0 }), if is_err { "" } else { out_text }, if is_err { out_text } else { "" }, start.elapsed().as_millis() as u64, if is_err { ValidationStatus::Failed } else { ValidationStatus::Success }, is_err, None, None, None).await;
            res
        }
        "file_edit" => {
            let path_str = arguments.get("path").and_then(|v| v.as_str()).unwrap_or("");
            let old_str = arguments.get("old_string").and_then(|v| v.as_str()).unwrap_or("");
            let new_str = arguments.get("new_string").and_then(|v| v.as_str()).unwrap_or("");
            let replace_all = arguments.get("replace_all").and_then(|v| v.as_bool()).unwrap_or(false);

            let res = match crate::fs::edit_file(path_str, old_str, new_str, replace_all) {
                Ok(edit_res) => {
                    let display_text = edit_res.to_string();
                    let mut tool_res = CallToolResult::success(display_text);
                    tool_res.meta = Some(serde_json::json!({
                        "diffs": [
                            {
                                "path": edit_res.path,
                                "oldText": edit_res.old_text,
                                "newText": edit_res.new_text,
                            }
                        ]
                    }));
                    tool_res
                }
                Err(err) => CallToolResult::error(err),
            };
            let is_err = res.is_error;
            let out_text = res.content.first().map(|c| c.text.as_str()).unwrap_or("");
            record_call(state, tool_name, arguments, RiskLevel::Medium, PolicyAction::Allow, Some(if is_err { 1 } else { 0 }), if is_err { "" } else { out_text }, if is_err { out_text } else { "" }, start.elapsed().as_millis() as u64, if is_err { ValidationStatus::Failed } else { ValidationStatus::Success }, is_err, None, None, None).await;
            res
        }
        "file_write" => {
            let path_str = arguments.get("path").and_then(|v| v.as_str()).unwrap_or("");
            let content = arguments.get("content").and_then(|v| v.as_str()).unwrap_or("");
            let create_dirs = arguments.get("create_dirs").and_then(|v| v.as_bool()).unwrap_or(true);

            let old_content_opt = std::fs::read_to_string(path_str).ok();

            let res = match crate::fs::write_file(path_str, content, create_dirs) {
                Ok(msg) => {
                    if let Some(old_content) = old_content_opt {
                        if old_content != content {
                            let diff = crate::fs::generate_unified_diff(path_str, &old_content, content);
                            let text = if !diff.is_empty() {
                                format!("{}\n\n{}", msg, diff)
                            } else {
                                msg
                            };
                            let mut tool_res = CallToolResult::success(text);
                            tool_res.meta = Some(serde_json::json!({
                                "diffs": [
                                    {
                                        "path": path_str,
                                        "oldText": old_content,
                                        "newText": content,
                                    }
                                ]
                            }));
                            tool_res
                        } else {
                            CallToolResult::success(msg)
                        }
                    } else {
                        CallToolResult::success(msg)
                    }
                }
                Err(err) => CallToolResult::error(err),
            };
            let is_err = res.is_error;
            let out_text = res.content.first().map(|c| c.text.as_str()).unwrap_or("");
            record_call(state, tool_name, arguments, RiskLevel::Medium, PolicyAction::Allow, Some(if is_err { 1 } else { 0 }), if is_err { "" } else { out_text }, if is_err { out_text } else { "" }, start.elapsed().as_millis() as u64, if is_err { ValidationStatus::Failed } else { ValidationStatus::Success }, is_err, None, None, None).await;
            res
        }
        "file_search" => {
            let path_str = arguments.get("path").and_then(|v| v.as_str()).unwrap_or(".");
            let pattern = arguments.get("pattern").and_then(|v| v.as_str()).unwrap_or("");
            let glob = arguments.get("glob").and_then(|v| v.as_str());
            let max_results = arguments.get("max_results").and_then(|v| v.as_u64()).map(|v| v as usize);
            let case_sensitive = arguments.get("case_sensitive").and_then(|v| v.as_bool());

            let res = match crate::fs::search_files(path_str, pattern, glob, max_results, case_sensitive) {
                Ok(matches) => CallToolResult::success(matches),
                Err(err) => CallToolResult::error(err),
            };
            let is_err = res.is_error;
            let out_text = res.content.first().map(|c| c.text.as_str()).unwrap_or("");
            record_call(state, tool_name, arguments, RiskLevel::Low, PolicyAction::Allow, Some(if is_err { 1 } else { 0 }), if is_err { "" } else { out_text }, if is_err { out_text } else { "" }, start.elapsed().as_millis() as u64, if is_err { ValidationStatus::Failed } else { ValidationStatus::Success }, is_err, None, None, None).await;
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
    let project = std::env::current_dir()
        .map(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or_else(|| p.to_str().unwrap_or("."))
                .to_string()
        })
        .unwrap_or_else(|_| "unknown".to_string());

    let rec = ExecutionRecord {
        id: format!("rec-{}", now_timestamp_nanos()),
        timestamp: now_timestamp(),
        source: "stdio".to_string(),
        project,
        pid: std::process::id(),
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

pub fn format_terminal_output(
    stdout: &str,
    stderr: &str,
    exit_code: Option<i32>,
    timed_out: bool,
) -> String {
    let mut out = String::new();
    if !stdout.is_empty() {
        out.push_str(stdout);
    }
    if !stderr.is_empty() {
        if !out.is_empty() && !out.ends_with('\n') {
            out.push('\n');
        }
        out.push_str("[stderr]\n");
        out.push_str(stderr);
    }
    if timed_out {
        if !out.is_empty() && !out.ends_with('\n') {
            out.push('\n');
        }
        out.push_str("[command timed out]");
    } else if let Some(code) = exit_code {
        if code != 0 {
            if !out.is_empty() && !out.ends_with('\n') {
                out.push('\n');
            }
            out.push_str(&format!("[exit code: {}]", code));
        }
    }
    if out.is_empty() {
        "[no output]".to_string()
    } else {
        out
    }
}
