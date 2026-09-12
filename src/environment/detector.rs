use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};
use sysinfo::System;

use super::fingerprint::EnvironmentFingerprint;

pub async fn detect_environment() -> EnvironmentFingerprint {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let platform = std::env::consts::OS.to_string();
    let architecture = std::env::consts::ARCH.to_string();

    let (distribution, version, kernel) = detect_os_details(&platform);
    let (shell, available_shells) = detect_shells(&platform);
    let (package_manager, available_pms) = detect_package_managers(&platform);
    let init_system = detect_init_system(&platform);
    let (user, is_admin) = detect_user_and_privileges(&platform).await;
    let is_container = detect_container();
    let docker_available = is_command_in_path("docker") || is_command_in_path("docker.exe");
    let hostname = System::host_name().unwrap_or_else(|| "localhost".to_string());

    EnvironmentFingerprint {
        platform,
        distribution,
        version,
        kernel,
        architecture,
        shell,
        available_shells,
        init_system,
        package_manager,
        available_package_managers: available_pms,
        user,
        is_admin,
        is_container,
        docker_available,
        hostname,
        timestamp: now,
        cache_age_ms: 0,
    }
}

fn detect_os_details(platform: &str) -> (String, String, String) {
    if platform == "windows" {
        let kernel = System::kernel_version().unwrap_or_else(|| "NT".to_string());
        let version = System::os_version().unwrap_or_else(|| "Windows".to_string());
        let dist = if version.contains("11") || kernel.contains("22") || kernel.contains("26") {
            "Windows 11".to_string()
        } else if version.contains("10") {
            "Windows 10".to_string()
        } else {
            "Windows".to_string()
        };
        (dist, version, kernel)
    } else if platform == "linux" {
        // Detect Termux first
        if Path::new("/data/data/com.termux").exists() || std::env::var("TERMUX_VERSION").is_ok() {
            let kernel = System::kernel_version().unwrap_or_default();
            return ("Termux (Android)".to_string(), "Android".to_string(), kernel);
        }

        // Parse /etc/os-release
        let mut dist = "Linux".to_string();
        let mut version = "unknown".to_string();
        if let Ok(content) = std::fs::read_to_string("/etc/os-release") {
            for line in content.lines() {
                if let Some(name) = line.strip_prefix("NAME=") {
                    dist = name.trim_matches('"').to_string();
                } else if let Some(ver) = line.strip_prefix("VERSION_ID=") {
                    version = ver.trim_matches('"').to_string();
                }
            }
        }
        let kernel = System::kernel_version().unwrap_or_default();
        (dist, version, kernel)
    } else {
        (
            platform.to_string(),
            System::os_version().unwrap_or_default(),
            System::kernel_version().unwrap_or_default(),
        )
    }
}

fn detect_shells(platform: &str) -> (String, Vec<String>) {
    let mut available = Vec::new();

    if platform == "windows" {
        if is_command_in_path("pwsh.exe") || is_command_in_path("pwsh") {
            available.push("pwsh".to_string());
        }
        if is_command_in_path("powershell.exe") {
            available.push("powershell".to_string());
        }
        if is_command_in_path("cmd.exe") {
            available.push("cmd".to_string());
        }
        if is_command_in_path("bash.exe") {
            available.push("bash".to_string());
        }

        let default_shell = if available.contains(&"pwsh".to_string()) {
            "pwsh".to_string()
        } else if available.contains(&"powershell".to_string()) {
            "powershell".to_string()
        } else {
            available.first().cloned().unwrap_or_else(|| "cmd".to_string())
        };

        (default_shell, available)
    } else {
        let shells_to_probe = ["bash", "zsh", "sh", "fish"];
        for s in shells_to_probe {
            if is_command_in_path(s) {
                available.push(s.to_string());
            }
        }

        let default_shell = if let Ok(sh_env) = std::env::var("SHELL") {
            let name = Path::new(&sh_env).file_name().and_then(|n| n.to_str()).unwrap_or("sh");
            name.to_string()
        } else if available.contains(&"bash".to_string()) {
            "bash".to_string()
        } else {
            available.first().cloned().unwrap_or_else(|| "sh".to_string())
        };

        (default_shell, available)
    }
}

fn detect_package_managers(platform: &str) -> (String, Vec<String>) {
    let mut available = Vec::new();

    if platform == "windows" {
        let pms = [("winget", "winget.exe"), ("choco", "choco.exe"), ("scoop", "scoop.cmd")];
        for (name, bin) in pms {
            if is_command_in_path(bin) || is_command_in_path(name) {
                available.push(name.to_string());
            }
        }
    } else {
        let pms = ["apt", "dnf", "yum", "pacman", "apk", "zypper", "pkg", "brew"];
        for name in pms {
            if is_command_in_path(name) {
                available.push(name.to_string());
            }
        }
    }

    let default_pm = available.first().cloned().unwrap_or_else(|| "none".to_string());
    (default_pm, available)
}

fn detect_init_system(platform: &str) -> String {
    if platform == "windows" {
        "windows-services".to_string()
    } else if Path::new("/run/systemd/system").exists() || is_command_in_path("systemctl") {
        "systemd".to_string()
    } else if Path::new("/sbin/openrc").exists() || Path::new("/etc/init.d").exists() {
        "openrc".to_string()
    } else if Path::new("/data/data/com.termux").exists() {
        "termux-services".to_string()
    } else {
        "unknown".to_string()
    }
}

async fn detect_user_and_privileges(platform: &str) -> (String, bool) {
    let user = std::env::var("USERNAME")
        .or_else(|_| std::env::var("USER"))
        .unwrap_or_else(|_| "unknown".to_string());

    if platform == "windows" {
        // Quick check for admin on Windows by attempting to check session or test-path
        let is_admin = tokio::process::Command::new("net.exe")
            .arg("session")
            .output()
            .await
            .map(|out| out.status.success())
            .unwrap_or(false);
        (user, is_admin)
    } else {
        #[cfg(unix)]
        {
            let is_root = unsafe { libc::geteuid() == 0 };
            (user, is_root)
        }
        #[cfg(not(unix))]
        {
            (user, false)
        }
    }
}

fn detect_container() -> bool {
    Path::new("/.dockerenv").exists()
        || Path::new("/run/.containerenv").exists()
        || std::env::var("container").is_ok()
}

pub fn is_command_in_path(cmd: &str) -> bool {
    if let Ok(path_var) = std::env::var("PATH") {
        let separator = if cfg!(windows) { ';' } else { ':' };
        for dir in path_var.split(separator) {
            let p = Path::new(dir).join(cmd);
            if p.is_file() {
                return true;
            }
            if cfg!(windows) && !cmd.ends_with(".exe") && !cmd.ends_with(".cmd") && !cmd.ends_with(".bat") {
                for ext in &[".exe", ".cmd", ".bat"] {
                    let with_ext = Path::new(dir).join(format!("{}{}", cmd, ext));
                    if with_ext.is_file() {
                        return true;
                    }
                }
            }
        }
    }
    false
}
