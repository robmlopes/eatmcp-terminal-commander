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
        || exit_code == Some(127)
        || exit_code == Some(9009)
    {
        let hint = if platform == "windows" && (program == "systemctl" || program == "apt") {
            format!("'{}' is a Linux command but environment is Windows.", program)
        } else {
            format!("Binary '{}' is not installed or not in PATH.", program)
        };
        return Diagnosis {
            error_class: ErrorClass::CommandNotFound,
            likely_cause: hint,
            suggested_next_action: "Call environment.inspect or environment.capabilities to check available tools."
                .to_string(),
        };
    }

    if lower_err.contains("access is denied")
        || lower_err.contains("permission denied")
        || lower_err.contains("run as administrator")
        || lower_err.contains("requires elevation")
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
