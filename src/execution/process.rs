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
            if args.is_empty() {
                c.args(&["/s", "/c", program]);
            } else {
                let mut combined_args = resolution.args_prefix;
                let clean_args: Vec<String> = args.iter().map(|a| clean_unquoted_path(a)).collect();
                combined_args.extend(clean_args);
                c.args(&combined_args);
            }
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
            let target_prog = &clean_prog;
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
            c.args(&["-NoLogo", "-NoProfile", "-NonInteractive", "-Command", &full_cmd]);
            c
        }
        "bash" | "sh" => {
            let bash_bin = if cfg!(windows) {
                if let Some(b) = crate::execution::resolver::find_windows_bash() {
                    b
                } else {
                    "bash".to_string()
                }
            } else if crate::environment::detector::is_command_in_path("bash") {
                "bash".to_string()
            } else {
                "sh".to_string()
            };
            let mut c = Command::new(&bash_bin);
            let full_cmd = if args.is_empty() {
                program.to_string()
            } else {
                let clean_args: Vec<String> = args.iter().map(|a| clean_unquoted_path(a)).collect();
                let joined_args: String = clean_args
                    .iter()
                    .map(|a| if a.contains(' ') && !(a.starts_with('"') && a.ends_with('"')) { format!("\"{}\"", a) } else { a.clone() })
                    .collect::<Vec<_>>()
                    .join(" ");
                format!("{} {}", clean_prog, joined_args)
            };
            c.args(&["-c", &full_cmd]);
            c
        }
        _ => {
            if args.is_empty() && (program.contains(' ') || program.contains(';') || program.contains('|') || program.contains('&')) {
                let shell_bin = if cfg!(windows) { "cmd.exe" } else { "bash" };
                let flag = if cfg!(windows) { "/c" } else { "-c" };
                let mut c = Command::new(shell_bin);
                c.args(&[flag, program]);
                c
            } else if let Some(ref exec) = resolution.resolved_executable {
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

    // Enhance PATH with Windows directories on WSL/Linux if available
    if !cfg!(windows) {
        if let Ok(current_path) = std::env::var("PATH") {
            let win_dirs = [
                "/mnt/c/Windows/System32",
                "/mnt/c/Windows",
                "/mnt/c/Windows/System32/Wbem",
                "/mnt/c/Windows/System32/WindowsPowerShell/v1.0",
            ];
            let mut to_add = Vec::new();
            for dir in &win_dirs {
                if std::path::Path::new(dir).is_dir() && !current_path.split(':').any(|p| p == *dir) {
                    to_add.push(*dir);
                }
            }
            if !to_add.is_empty() {
                let enriched_path = format!("{}:{}", current_path, to_add.join(":"));
                cmd.env("PATH", enriched_path);
            }
        }
    }

    cmd.stdin(Stdio::null());
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    // 2. Spawn process
    let child_res = cmd.spawn();
    let mut child = match child_res {
        Ok(c) => c,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            // Automatic fallback through shell
            let (fallback_shell, flag) = if cfg!(windows) { ("cmd.exe", "/c") } else { ("bash", "-c") };
            let full_line = if args.is_empty() {
                program.to_string()
            } else {
                let clean_args: Vec<String> = args.iter().map(|a| clean_unquoted_path(a)).collect();
                let joined_args: String = clean_args
                    .iter()
                    .map(|a| if a.contains(' ') && !(a.starts_with('"') && a.ends_with('"')) { format!("\"{}\"", a) } else { a.clone() })
                    .collect::<Vec<_>>()
                    .join(" ");
                format!("{} {}", program, joined_args)
            };
            let mut fallback_cmd = Command::new(fallback_shell);
            fallback_cmd.args(&[flag, &full_line]);
            if let Some(wd) = working_dir {
                fallback_cmd.current_dir(clean_unquoted_path(wd));
            }
            if !cfg!(windows) {
                if let Ok(current_path) = std::env::var("PATH") {
                    let win_dirs = [
                        "/mnt/c/Windows/System32",
                        "/mnt/c/Windows",
                        "/mnt/c/Windows/System32/Wbem",
                    ];
                    let mut to_add = Vec::new();
                    for dir in &win_dirs {
                        if std::path::Path::new(dir).is_dir() && !current_path.split(':').any(|p| p == *dir) {
                            to_add.push(*dir);
                        }
                    }
                    if !to_add.is_empty() {
                        fallback_cmd.env("PATH", format!("{}:{}", current_path, to_add.join(":")));
                    }
                }
            }
            fallback_cmd.stdin(Stdio::null());
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
