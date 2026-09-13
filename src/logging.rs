use std::fs::{create_dir_all, OpenOptions};
use std::io::Write;
use std::path::Path;

use crate::state::ExecutionRecord;

pub fn log_execution(record: &ExecutionRecord) {
    let log_dir = Path::new("logs");
    let _ = create_dir_all(log_dir);

    // Ensure .gitignore exists in logs/
    let gitignore = log_dir.join(".gitignore");
    if !gitignore.exists() {
        let _ = std::fs::write(&gitignore, "*\n!.gitignore\n");
    }

    // 1. General execution log (concise single-line per command)
    if let Ok(mut file) = OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_dir.join("execution.log"))
    {
        let time_str = format_timestamp(record.timestamp);
        let cmd = record.arguments.get("command")
            .and_then(|v| v.as_str())
            .unwrap_or(&record.tool_name);

        let _ = writeln!(
            file,
            "[{}] [PID: {}] [Project: {}] Tool: {} | Cmd: '{}' | Exit: {:?} | {}ms | Status: {}",
            time_str,
            record.pid,
            if record.project.is_empty() { "default" } else { &record.project },
            record.tool_name,
            cmd,
            record.exit_code,
            record.duration_ms,
            record.validation_status
        );
    }

    // 2. Failed commands log (detailed debug block)
    if record.is_error {
        if let Ok(mut file) = OpenOptions::new()
            .create(true)
            .append(true)
            .open(log_dir.join("failed_commands.log"))
        {
            let time_str = format_timestamp(record.timestamp);
            let cmd = record.arguments.get("command")
                .and_then(|v| v.as_str())
                .unwrap_or(&record.tool_name);

            let _ = writeln!(file, "{}", "=".repeat(80));
            let _ = writeln!(file, "Timestamp: {}", time_str);
            let _ = writeln!(file, "PID: {} | Project: {}", record.pid, record.project);
            let _ = writeln!(file, "Tool: {} | Command: '{}'", record.tool_name, cmd);
            let _ = writeln!(file, "Arguments: {}", serde_json::to_string(&record.arguments).unwrap_or_default());
            let _ = writeln!(file, "Exit Code: {:?}", record.exit_code);
            let _ = writeln!(file, "Error Class: {}", record.error_class.as_deref().unwrap_or("UNKNOWN"));
            let _ = writeln!(file, "Likely Cause: {}", record.likely_cause.as_deref().unwrap_or("n/a"));
            let _ = writeln!(file, "Suggested Action: {}", record.suggested_next_action.as_deref().unwrap_or("n/a"));
            if !record.stderr.is_empty() {
                let _ = writeln!(file, "--- Stderr ---\n{}", record.stderr.trim());
            }
            if !record.stdout.is_empty() {
                let _ = writeln!(file, "--- Stdout ---\n{}", record.stdout.trim());
            }
            let _ = writeln!(file, "{}\n", "=".repeat(80));
        }
    }
}

fn format_timestamp(ts: u64) -> String {
    // Return formatted timestamp
    let dt = match std::time::UNIX_EPOCH.checked_add(std::time::Duration::from_secs(ts)) {
        Some(t) => t,
        None => std::time::SystemTime::now(),
    };
    let dur = dt.duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
    let total_secs = dur.as_secs();
    let hours = (total_secs / 3600) % 24;
    let minutes = (total_secs / 60) % 60;
    let seconds = total_secs % 60;
    format!("{}s ({:02}:{:02}:{:02} UTC)", total_secs, hours, minutes, seconds)
}
