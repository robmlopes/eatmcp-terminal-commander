use std::path::{Path, PathBuf};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShellCompatibility {
    pub cmd: bool,
    pub powershell: bool,
    pub bash: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandResolution {
    pub original_command: String,
    pub resolved_executable: Option<String>,
    pub args_prefix: Vec<String>,
    pub resolution_source: String,
    pub recommended_shell: String, // "direct", "cmd", "powershell", "bash"
    pub shell_compatibility: ShellCompatibility,
}

pub fn resolve_command(raw_program: &str) -> CommandResolution {
    // 1. Clean any surrounding or inner quotes (e.g. C:\"Program Files"\... -> C:\Program Files\...)
    let clean_program = clean_unquoted_path(raw_program);
    let lower = clean_program.to_lowercase();

    // 2. Check CMD Built-ins (dir, copy, type, del, etc.)
    if is_cmd_builtin(&lower) {
        return CommandResolution {
            original_command: raw_program.to_string(),
            resolved_executable: Some("cmd.exe".to_string()),
            args_prefix: vec!["/c".to_string(), clean_program.clone()],
            resolution_source: "cmd_builtin".to_string(),
            recommended_shell: "cmd".to_string(),
            shell_compatibility: ShellCompatibility {
                cmd: true,
                powershell: false,
                bash: false,
                notes: Some(format!("'{}' is an internal CMD command. Must be executed via cmd.exe.", clean_program)),
            },
        };
    }

    // 3. Check PowerShell Cmdlets (Get-Process, Test-Path, etc.)
    if is_powershell_cmdlet(&lower) {
        return CommandResolution {
            original_command: raw_program.to_string(),
            resolved_executable: Some(if is_command_in_path("pwsh.exe") { "pwsh.exe".to_string() } else { "powershell.exe".to_string() }),
            args_prefix: vec!["-NoProfile".to_string(), "-NonInteractive".to_string(), "-Command".to_string(), clean_program.clone()],
            resolution_source: "powershell_cmdlet".to_string(),
            recommended_shell: "powershell".to_string(),
            shell_compatibility: ShellCompatibility {
                cmd: false,
                powershell: true,
                bash: false,
                notes: Some(format!("'{}' is a PowerShell cmdlet. Must be executed via PowerShell / pwsh.", clean_program)),
            },
        };
    }

    // 4. Check Environment Variables for Program Mapping (case-insensitive on Windows)
    if let Some((path_str, var_name)) = find_in_env_variables(&clean_program) {
        let (shell, prefix) = determine_exec_wrapper(&path_str, &clean_program);
        return CommandResolution {
            original_command: raw_program.to_string(),
            resolved_executable: Some(path_str.clone()),
            args_prefix: prefix,
            resolution_source: format!("env_variable({})", var_name),
            recommended_shell: shell,
            shell_compatibility: analyze_compatibility(&path_str, &clean_program),
        };
    }

    // 5. Check Standard PATH + PATHEXT
    if let Some((path_str, _ext_kind)) = search_path_with_pathext(&clean_program) {
        let (shell, prefix) = determine_exec_wrapper(&path_str, &clean_program);

        return CommandResolution {
            original_command: raw_program.to_string(),
            resolved_executable: Some(path_str.clone()),
            args_prefix: prefix,
            resolution_source: "pathext".to_string(),
            recommended_shell: shell,
            shell_compatibility: analyze_compatibility(&path_str, &clean_program),
        };
    }

    // 6. Check System Directories on Windows (System32, Sysnative)
    if cfg!(windows) {
        if let Some(sys_path) = find_in_system_dirs(&clean_program) {
            let path_str = sys_path.to_string_lossy().to_string();
            let (shell, prefix) = determine_exec_wrapper(&path_str, &clean_program);

            return CommandResolution {
                original_command: raw_program.to_string(),
                resolved_executable: Some(path_str.clone()),
                args_prefix: prefix,
                resolution_source: "system_dir".to_string(),
                recommended_shell: shell,
                shell_compatibility: analyze_compatibility(&path_str, &clean_program),
            };
        }
    }

    // 7. Fallback: Not resolved on disk
    CommandResolution {
        original_command: raw_program.to_string(),
        resolved_executable: None,
        args_prefix: vec![],
        resolution_source: "unknown".to_string(),
        recommended_shell: if cfg!(windows) { "cmd".to_string() } else { "sh".to_string() },
        shell_compatibility: ShellCompatibility {
            cmd: false,
            powershell: false,
            bash: false,
            notes: Some(format!("Program '{}' was not found in PATH, environment mappings, or system directories.", clean_program)),
        },
    }
}

pub fn clean_unquoted_path(path: &str) -> String {
    path.trim().replace('"', "")
}

pub fn normalize_quoted_command(path: &str) -> String {
    let clean = clean_unquoted_path(path);
    if clean.contains(' ') {
        format!("\"{}\"", clean)
    } else {
        clean
    }
}

pub fn normalize_quoted_arg(arg: &str) -> String {
    let trimmed = arg.trim();
    // If argument has inner quotes (e.g. C:\"Program Files"\foo)
    if (trimmed.contains(r#"\""#) || trimmed.contains(r#"/"#) || trimmed.contains(r#":\"#)) && trimmed.contains('"') {
        let clean = trimmed.replace('"', "");
        if clean.contains(' ') {
            return format!("\"{}\"", clean);
        }
        return clean;
    }

    // If it contains spaces and is not already completely enclosed in double quotes
    if trimmed.contains(' ') && !(trimmed.starts_with('"') && trimmed.ends_with('"')) {
        format!("\"{}\"", trimmed)
    } else {
        trimmed.to_string()
    }
}

fn is_cmd_builtin(cmd: &str) -> bool {
    matches!(
        cmd,
        "dir" | "copy" | "move" | "del" | "erase" | "ren" | "rename" | "type" | "cls"
            | "ver" | "vol" | "assoc" | "ftype" | "mklink" | "md" | "mkdir" | "rd" | "rmdir"
            | "chdir" | "attrib" | "color" | "title"
    )
}

fn is_powershell_cmdlet(cmd: &str) -> bool {
    matches!(
        cmd,
        "get-process" | "get-service" | "get-content" | "start-process" | "stop-process"
            | "test-path" | "get-childitem" | "set-content" | "new-item" | "remove-item"
            | "select-object" | "where-object" | "invoke-webrequest" | "invoke-restmethod"
            | "out-string" | "export-csv" | "get-item" | "set-item"
    )
}

fn find_in_env_variables(program: &str) -> Option<(String, String)> {
    let upper = program.to_uppercase();

    // Check direct env var matching PROGRAM
    if let Some((val, key)) = get_env_var_matching(&[program, &upper]) {
        let p = PathBuf::from(&val);
        if p.is_file() {
            return Some((p.to_string_lossy().to_string(), key));
        }
    }

    // Suffix variants: PROGRAM_PATH, PROGRAM_EXE, PROGRAM_BIN
    for suffix in &["_PATH", "_EXE", "_BIN"] {
        let var_name = format!("{}{}", upper, suffix);
        if let Some((val, key)) = get_env_var_matching(&[&var_name]) {
            let p = PathBuf::from(&val);
            if p.is_file() {
                return Some((p.to_string_lossy().to_string(), key));
            }
        }
    }

    // Home directory variants: PROGRAM_HOME (e.g. JAVA_HOME, NVM_SYMLINK, HIP_PATH, etc.)
    let home_vars = [
        format!("{}_HOME", upper),
        "JAVA_HOME".to_string(),
        "NVM_SYMLINK".to_string(),
        "NVM_HOME".to_string(),
        "ChocolateyInstall".to_string(),
        "HIP_PATH".to_string(),
        "AMDRMPATH".to_string(),
    ];

    for var_name in &home_vars {
        if let Some((home_val, key)) = get_env_var_matching(&[var_name]) {
            let home_path = PathBuf::from(home_val);
            for sub in &["", "bin", "cmd", "usr/bin"] {
                let dir = if sub.is_empty() { home_path.clone() } else { home_path.join(sub) };
                if let Some(found) = probe_extensions(&dir, program) {
                    return Some((found.to_string_lossy().to_string(), key));
                }
            }
        }
    }

    None
}

fn get_env_var_matching(keys: &[&str]) -> Option<(String, String)> {
    if cfg!(windows) {
        // Windows: case-insensitive match
        for (k, v) in std::env::vars() {
            for expected in keys {
                if k.eq_ignore_ascii_case(expected) {
                    return Some((v, k));
                }
            }
        }
    } else {
        // Linux: exact case-sensitive match
        for expected in keys {
            if let Ok(v) = std::env::var(expected) {
                return Some((v, expected.to_string()));
            }
        }
    }
    None
}

fn search_path_with_pathext(program: &str) -> Option<(String, String)> {
    let path_var = if cfg!(windows) {
        std::env::vars()
            .find(|(k, _)| k.eq_ignore_ascii_case("PATH"))
            .map(|(_, v)| v)
    } else {
        std::env::var("PATH").ok()
    }?;

    let separator = if cfg!(windows) { ';' } else { ':' };
    for dir in path_var.split(separator) {
        let d = Path::new(dir);
        if let Some(found) = probe_extensions(d, program) {
            let ext = found
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_lowercase();
            return Some((found.to_string_lossy().to_string(), ext));
        }
    }
    None
}

fn find_in_system_dirs(program: &str) -> Option<PathBuf> {
    if !cfg!(windows) {
        return None;
    }

    let sys_root = std::env::var("SystemRoot")
        .or_else(|_| std::env::var("windir"))
        .unwrap_or_else(|_| "C:\\Windows".to_string());

    let base = Path::new(&sys_root);
    let dirs = [
        base.join("System32"),
        base.join("Sysnative"),
        base.join("SysWOW64"),
        base.to_path_buf(),
    ];

    for d in &dirs {
        if let Some(found) = probe_extensions(d, program) {
            return Some(found);
        }
    }

    None
}

fn probe_extensions(dir: &Path, program: &str) -> Option<PathBuf> {
    let clean = clean_unquoted_path(program);
    let lower = clean.to_lowercase();

    // 1. If program already carries a known executable extension, check it directly
    if lower.ends_with(".exe") || lower.ends_with(".cmd") || lower.ends_with(".bat") || lower.ends_with(".ps1") {
        let p = dir.join(&clean);
        if p.is_file() {
            return Some(p);
        }
    }

    // 2. On Windows: prioritize Win32 executable extensions (.exe, .cmd, .bat, .ps1, .com) OVER extensionless files
    if cfg!(windows) {
        for ext in &[".exe", ".cmd", ".bat", ".ps1", ".com"] {
            let with_ext = dir.join(format!("{}{}", clean, ext));
            if with_ext.is_file() {
                return Some(with_ext);
            }
        }
    }

    // 3. Exact match without extension (POSIX binary or direct match)
    let p = dir.join(&clean);
    if p.is_file() {
        return Some(p);
    }

    None
}

fn determine_exec_wrapper(path_str: &str, program: &str) -> (String, Vec<String>) {
    let clean = clean_unquoted_path(path_str);
    let lower_path = clean.to_lowercase();
    let lower_name = program.to_lowercase();

    if lower_path.ends_with(".cmd") || lower_path.ends_with(".bat") {
        ("cmd".to_string(), vec!["/c".to_string(), clean])
    } else if lower_path.ends_with(".ps1") {
        ("powershell".to_string(), vec!["-NoProfile".to_string(), "-ExecutionPolicy".to_string(), "Bypass".to_string(), "-File".to_string(), clean])
    } else if lower_name == "telnet" {
        ("cmd".to_string(), vec!["/c".to_string(), clean])
    } else {
        ("direct".to_string(), vec![])
    }
}

fn analyze_compatibility(path_str: &str, program: &str) -> ShellCompatibility {
    let lower_path = path_str.to_lowercase();
    let lower_name = program.to_lowercase();

    if lower_path.ends_with(".cmd") || lower_path.ends_with(".bat") {
        ShellCompatibility {
            cmd: true,
            powershell: true,
            bash: false,
            notes: Some("Windows batch script (.cmd/.bat). Executes natively in CMD and PowerShell; fails in POSIX Bash.".to_string()),
        }
    } else if lower_path.ends_with(".ps1") {
        ShellCompatibility {
            cmd: false,
            powershell: true,
            bash: false,
            notes: Some("PowerShell script (.ps1). Must be executed via PowerShell.".to_string()),
        }
    } else if lower_name == "telnet" {
        ShellCompatibility {
            cmd: true,
            powershell: true,
            bash: false,
            notes: Some("Telnet is an interactive Windows console application. Runs in CMD and PowerShell; fails or hangs in Bash/MSYS2 without winpty.".to_string()),
        }
    } else {
        ShellCompatibility {
            cmd: true,
            powershell: true,
            bash: true,
            notes: None,
        }
    }
}

fn is_command_in_path(cmd: &str) -> bool {
    search_path_with_pathext(cmd).is_some()
}
