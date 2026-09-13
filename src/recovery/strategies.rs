use super::errors::ErrorClass;

pub struct Diagnosis {
    pub error_class: ErrorClass,
    pub likely_cause: String,
    pub suggested_next_action: String,
}

pub fn diagnose_failure(
    program: &str,
    exit_code: Option<i32>,
    stderr: &str,
    timed_out: bool,
    platform: &str,
) -> Diagnosis {
    let lower_err = stderr.to_lowercase();

    if timed_out {
        return Diagnosis {
            error_class: ErrorClass::Timeout,
            likely_cause: format!("Command '{}' exceeded execution timeout window", program),
            suggested_next_action: "Increase timeout_ms parameter or execute as background process".to_string(),
        };
    }

    if lower_err.contains("not recognized")
        || lower_err.contains("command not found")
        || lower_err.contains("no such file or directory")
        || lower_err.contains("não é reconhecido")
        || lower_err.contains("nao e reconhecido")
        || exit_code == Some(127)
        || exit_code == Some(9009)
    {
        let hint = if platform == "windows" && (program == "systemctl" || program == "apt") {
            format!("'{}' is a Linux command but environment is Windows.", program)
        } else {
            format!("Binary or command '{}' is not recognized in current shell or not in PATH.", program)
        };
        return Diagnosis {
            error_class: ErrorClass::CommandNotFound,
            likely_cause: hint,
            suggested_next_action: suggest_missing_program(program, platform),
        };
    }

    if lower_err.contains("access is denied")
        || lower_err.contains("permission denied")
        || lower_err.contains("run as administrator")
        || lower_err.contains("requires elevation")
        || lower_err.contains("acesso negado")
        || exit_code == Some(5)
        || exit_code == Some(126)
    {
        return Diagnosis {
            error_class: ErrorClass::PermissionDenied,
            likely_cause: "Current execution context lacks administrative / root privileges.".to_string(),
            suggested_next_action: if platform == "windows" {
                "Run terminal as Administrator or execute with elevated credentials.".to_string()
            } else {
                "Prepend command with sudo or switch to root user.".to_string()
            },
        };
    }

    if lower_err.contains("does not exist as an installed service")
        || lower_err.contains("unit not found")
        || lower_err.contains("failed to get properties")
    {
        return Diagnosis {
            error_class: ErrorClass::ServiceMissing,
            likely_cause: "Target service name was not found in service manager database.".to_string(),
            suggested_next_action: "Verify the exact service name or install corresponding package.".to_string(),
        };
    }

    if lower_err.contains("no package found")
        || lower_err.contains("unable to locate package")
        || lower_err.contains("no package matches")
    {
        return Diagnosis {
            error_class: ErrorClass::PackageNotFound,
            likely_cause: "Package not found in package manager repositories.".to_string(),
            suggested_next_action: "Update package repository cache or search for exact package identifier.".to_string(),
        };
    }

    Diagnosis {
        error_class: ErrorClass::GeneralFailure,
        likely_cause: format!(
            "Command exited with non-zero code {:?}. Stderr output: {}",
            exit_code,
            stderr.trim()
        ),
        suggested_next_action: "Inspect stderr output and command parameters.".to_string(),
    }
}

pub fn suggest_missing_program(program: &str, platform: &str) -> String {
    let lower = program.to_lowercase();
    if platform == "windows" {
        if lower == "ls" || lower == "ll" {
            return "On Windows, use 'dir' or run via PowerShell with use_shell: true.".to_string();
        }
        if lower == "cat" {
            return "On Windows, use 'type' in cmd or 'Get-Content' in PowerShell, or pass use_shell: true.".to_string();
        }
        if lower == "rm" {
            return "On Windows, use 'del' (files) or 'rmdir' (folders), or pass use_shell: true.".to_string();
        }
        if lower == "grep" {
            return "On Windows, use 'findstr' or 'Select-String' in PowerShell.".to_string();
        }
        if lower == "which" {
            return "On Windows, use 'where.exe' or 'Get-Command' in PowerShell.".to_string();
        }
        if lower == "sudo" {
            return "sudo is not native to Windows. Run terminal elevated as Administrator.".to_string();
        }
        if lower == "apt" || lower == "apt-get" || lower == "brew" || lower == "apk" {
            return "Linux/macOS package manager. On Windows, use 'winget' or 'choco'.".to_string();
        }
        if lower == "systemctl" || lower == "service" {
            return "systemd is not available on Windows. Use EATMCP 'service_status' / 'service_restart' or 'sc.exe'.".to_string();
        }
        if lower == "npm" || lower == "npx" || lower == "node" || lower == "yarn" || lower == "pnpm" {
            if std::path::Path::new(r"C:\Program Files\nodejs\npm.cmd").exists() {
                return format!("Node.js tools located in 'C:\\Program Files\\nodejs'. Try invoking as '{}.cmd' or pass use_shell: true.", program);
            }
            return "Node.js was not found in PATH. Install Node.js or verify environment PATH.".to_string();
        }
        if lower == "python" || lower == "python3" || lower == "pip" {
            return "Python was not found in PATH. Try 'py', or specify full python path.".to_string();
        }
        if lower == "cargo" || lower == "rustc" {
            return "Rust toolchain was not found in standard PATH. Verify ~/.cargo/bin or specify full path.".to_string();
        }

        format!("Command '{}' is not in PATH. Check if it requires a file extension (.exe, .cmd, .bat), or pass use_shell: true to invoke via system shell.", program)
    } else {
        format!("Binary '{}' not found in PATH. Verify installation or install with package manager.", program)
    }
}
