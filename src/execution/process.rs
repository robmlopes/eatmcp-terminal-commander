use std::process::Stdio;
use std::time::Instant;
use tokio::io::AsyncReadExt;
use tokio::process::Command;
use tokio::time::{timeout, Duration};

use super::resolver::{clean_unquoted_path, normalize_quoted_command, resolve_command};

#[derive(Debug, Clone)]
pub struct ProcessOutput {
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub duration_ms: u64,
    pub timed_out: bool,
}

pub async fn run_command(
    program: &str,
    args: &[String],
    working_dir: Option<&str>,
    timeout_ms: u64,
    requested_shell: &str,
) -> Result<ProcessOutput, std::io::Error> {
    let start_time = Instant::now();
    let clean_prog = clean_unquoted_path(program);
    let resolution = resolve_command(&clean_prog, requested_shell);

    // If POSIX tool was requested on Windows in auto mode and no bash was found
    if resolution.recommended_shell == "posix_unavailable" {
        return Ok(ProcessOutput {
            exit_code: Some(1),
            stdout: String::new(),
            stderr: format!(
                "'{}' is a POSIX/Linux command expecting Unix output. It is not available natively in Windows without Bash or WSL.\nTo get Windows native directory listing, run 'dir' (via CMD).\nIf PowerShell table output is acceptable, explicitly specify shell: 'powershell'.",
                clean_prog
            ),
            duration_ms: start_time.elapsed().as_millis() as u64,
            timed_out: false,
        });
    }

    if resolution.recommended_shell == "cmd_builtin_unsupported_on_linux" {
        return Ok(ProcessOutput {
            exit_code: Some(1),
            stdout: String::new(),
            stderr: format!(
                "'{}' is a Windows CMD internal command and does not exist on Linux. Use POSIX equivalent (e.g. 'ls' for dir, 'cat' for type).",
                clean_prog
            ),
            duration_ms: start_time.elapsed().as_millis() as u64,
            timed_out: false,
        });
    }

    // 1. Build the command based on resolution.recommended_shell
    let mut cmd = match resolution.recommended_shell.as_str() {
        "cmd" => {
            let mut c = Command::new("cmd.exe");
            let mut combined_args = resolution.args_prefix;
            let clean_args: Vec<String> = args.iter().map(|a| clean_unquoted_path(a)).collect();
            combined_args.extend(clean_args);
            c.args(&combined_args);
            c
        }
        "powershell" => {
            let shell_bin = if crate::environment::detector::is_command_in_path("pwsh.exe") {
                "pwsh.exe"
            } else {
                "powershell.exe"
            };
            let mut c = Command::new(shell_bin);
            let clean_args: Vec<String> = args.iter().map(|a| clean_unquoted_path(a)).collect();
            let target_prog = resolution.resolved_executable.as_deref().unwrap_or(&clean_prog);
            let quoted_target = normalize_quoted_command(target_prog);
            let joined_args: String = clean_args
                .iter()
                .map(|a| if a.contains(' ') { format!("\"{}\"", a) } else { a.clone() })
                .collect::<Vec<_>>()
                .join(" ");

            let full_cmd = if quoted_target.starts_with('"') && quoted_target.ends_with('"') {
                if joined_args.is_empty() {
                    format!("& {}", quoted_target)
                } else {
                    format!("& {} {}", quoted_target, joined_args)
                }
            } else {
                if joined_args.is_empty() {
                    quoted_target
                } else {
                    format!("{} {}", quoted_target, joined_args)
                }
            };
            c.args(&["-NoProfile", "-NonInteractive", "-Command", &full_cmd]);
            c
        }
        "bash" => {
            let bash_bin = if cfg!(windows) {
                if crate::environment::detector::is_command_in_path("bash.exe") {
                    "bash.exe".to_string()
                } else if std::path::Path::new(r"C:\Program Files\Git\bin\bash.exe").exists() {
                    r"C:\Program Files\Git\bin\bash.exe".to_string()
                } else {
                    "bash".to_string()
                }
            } else {
                "bash".to_string()
            };
            let mut c = Command::new(&bash_bin);
            let clean_args: Vec<String> = args.iter().map(|a| clean_unquoted_path(a)).collect();
            let target_prog = resolution.resolved_executable.as_deref().unwrap_or(&clean_prog);
            let full_cmd = if clean_args.is_empty() {
                target_prog.to_string()
            } else {
                format!("{} {}", target_prog, clean_args.join(" "))
            };
            c.args(&["-c", &full_cmd]);
            c
        }
        _ => {
            if let Some(ref exec) = resolution.resolved_executable {
                let clean_exec = clean_unquoted_path(exec);
                let mut c = Command::new(&clean_exec);
                let clean_args: Vec<String> = args.iter().map(|a| clean_unquoted_path(a)).collect();
                c.args(&clean_args);
                c
            } else {
                let mut c = Command::new(&clean_prog);
                let clean_args: Vec<String> = args.iter().map(|a| clean_unquoted_path(a)).collect();
                c.args(&clean_args);
                c
            }
        }
    };

    if let Some(wd) = working_dir {
        let clean_wd = clean_unquoted_path(wd);
        cmd.current_dir(clean_wd);
    }

    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    // 2. Spawn process
    let child_res = cmd.spawn();
    let mut child = match child_res {
        Ok(c) => c,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound && cfg!(windows) => {
            // Automatic fallback through cmd.exe /c
            let mut fallback_cmd = Command::new("cmd.exe");
            let mut fallback_args = vec!["/s".to_string(), "/c".to_string(), normalize_quoted_command(&clean_prog)];
            fallback_args.extend(args.iter().map(|a| clean_unquoted_path(a)));
            fallback_cmd.args(&fallback_args);
            if let Some(wd) = working_dir {
                fallback_cmd.current_dir(clean_unquoted_path(wd));
            }
            fallback_cmd.stdout(Stdio::piped());
            fallback_cmd.stderr(Stdio::piped());
            fallback_cmd.spawn().map_err(|_| err)?
        }
        Err(err) => return Err(err),
    };

    let stdout_handle = child.stdout.take();
    let stderr_handle = child.stderr.take();

    let stdout_task = tokio::spawn(async move {
        let mut buf = Vec::new();
        if let Some(mut r) = stdout_handle {
            let _ = r.read_to_end(&mut buf).await;
        }
        buf
    });

    let stderr_task = tokio::spawn(async move {
        let mut buf = Vec::new();
        if let Some(mut r) = stderr_handle {
            let _ = r.read_to_end(&mut buf).await;
        }
        buf
    });

    let time_limit = Duration::from_millis(if timeout_ms == 0 { 30_000 } else { timeout_ms });

    match timeout(time_limit, child.wait()).await {
        Ok(status_res) => {
            let status = status_res?;
            let stdout_bytes = stdout_task.await.unwrap_or_default();
            let stderr_bytes = stderr_task.await.unwrap_or_default();
            let duration_ms = start_time.elapsed().as_millis() as u64;
            let stdout = String::from_utf8_lossy(&stdout_bytes).to_string();
            let stderr = String::from_utf8_lossy(&stderr_bytes).to_string();

            Ok(ProcessOutput {
                exit_code: status.code(),
                stdout,
                stderr,
                duration_ms,
                timed_out: false,
            })
        }
        Err(_) => {
            let _ = child.kill().await;
            let duration_ms = start_time.elapsed().as_millis() as u64;
            Ok(ProcessOutput {
                exit_code: None,
                stdout: String::new(),
                stderr: "Execution timed out and process was killed".to_string(),
                duration_ms,
                timed_out: true,
            })
        }
    }
}
