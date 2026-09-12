use super::policy::{PolicyAction, RiskLevel};

pub struct SafetyAssessment {
    pub risk_level: RiskLevel,
    pub action: PolicyAction,
    pub reason: String,
}

pub fn classify_command(program: &str, args: &[String]) -> SafetyAssessment {
    let lower_prog = program.to_lowercase();
    let joined_args = args.join(" ").to_lowercase();
    let full_line = format!("{} {}", lower_prog, joined_args);

    // 1. Check CRITICAL (Always DENY)
    let critical_patterns = [
        "format.com",
        "diskpart",
        "mkfs",
        "fdisk",
        "parted",
        "dd if=/dev/zero",
        "dd if=/dev/urandom",
        ":(){ :|:& };:",
        "rm -rf /",
        "rm -rf /*",
        "del /s /q c:\\",
        "del /s /q c:/",
        "rd /s /q c:\\",
        "rd /s /q c:/",
        "/etc/shadow",
        "mimikatz",
        "Invoke-Mimikatz",
    ];

    for pattern in &critical_patterns {
        if full_line.contains(pattern) || lower_prog.ends_with(pattern) {
            return SafetyAssessment {
                risk_level: RiskLevel::Critical,
                action: PolicyAction::Deny,
                reason: format!("Destructive command or credential access detected matching '{}'", pattern),
            };
        }
    }

    // 2. Check HIGH Risk (Requires Confirmation)
    let high_patterns = [
        "rmdir /s",
        "rd /s",
        "del /s",
        "rm -r",
        "rm -f",
        "drop table",
        "drop database",
        "iptables -f",
        "ufw disable",
        "netsh advfirewall set allprofiles state off",
        "taskkill /f",
        "kill -9",
    ];

    for pattern in &high_patterns {
        if full_line.contains(pattern) {
            return SafetyAssessment {
                risk_level: RiskLevel::High,
                action: PolicyAction::Confirm,
                reason: format!("High-impact mutation detected matching '{}'", pattern),
            };
        }
    }

    // 3. Check MEDIUM Risk (Allowed for automated tasks, but recorded)
    let medium_programs = [
        "sc", "sc.exe", "systemctl", "service", "winget", "winget.exe",
        "apt", "apt-get", "dnf", "yum", "pacman", "apk", "pkg", "choco", "scoop",
        "docker", "podman", "git commit", "git push", "npm install", "cargo build",
    ];

    for mp in &medium_programs {
        if lower_prog.ends_with(mp) || lower_prog == *mp || full_line.starts_with(mp) {
            return SafetyAssessment {
                risk_level: RiskLevel::Medium,
                action: PolicyAction::Allow,
                reason: format!("System or package mutation via '{}'", mp),
            };
        }
    }

    // 4. Default: Safe inspect or general execution
    let low_programs = [
        "echo", "dir", "ls", "pwd", "cd", "cat", "type", "whoami", "hostname",
        "get-process", "ps", "ipconfig", "ifconfig", "ping", "tracert", "traceroute",
        "git status", "git log", "git diff", "findstr", "grep", "where", "which",
    ];

    for lp in &low_programs {
        if lower_prog == *lp || lower_prog.ends_with(lp) {
            return SafetyAssessment {
                risk_level: RiskLevel::Low,
                action: PolicyAction::Allow,
                reason: "Read-only inspection command".to_string(),
            };
        }
    }

    SafetyAssessment {
        risk_level: RiskLevel::Low,
        action: PolicyAction::Allow,
        reason: "Standard command execution".to_string(),
    }
}
