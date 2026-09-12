use std::process::Stdio;
use std::time::Instant;
use tokio::io::AsyncReadExt;
use tokio::process::Command;
use tokio::time::{timeout, Duration};

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
    use_shell: bool,
    shell: &str,
) -> Result<ProcessOutput, std::io::Error> {
    let start_time = Instant::now();

    let mut cmd = if use_shell {
        if cfg!(windows) {
            if shell == "cmd" {
                let mut c = Command::new("cmd.exe");
                let mut cmd_args = vec!["/c".to_string(), program.to_string()];
                cmd_args.extend_from_slice(args);
                c.args(&cmd_args);
                c
            } else {
                let shell_bin = if shell == "pwsh" { "pwsh.exe" } else { "powershell.exe" };
                let mut c = Command::new(shell_bin);
                let full_cmd = format!("{} {}", program, args.join(" "));
                c.args(&["-NoProfile", "-NonInteractive", "-Command", &full_cmd]);
                c
            }
        } else {
            let mut c = Command::new(if shell.is_empty() { "sh" } else { shell });
            let full_cmd = format!("{} {}", program, args.join(" "));
            c.args(&["-c", &full_cmd]);
            c
        }
    } else {
        let mut c = Command::new(program);
        c.args(args);
        c
    };

    if let Some(wd) = working_dir {
        cmd.current_dir(wd);
    }

    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    let mut child = cmd.spawn()?;
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
