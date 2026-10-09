use std::path::{Path, PathBuf};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CommandDialect {
    Posix,
    CmdBuiltin,
    PowerShell,
    WindowsBatch,
    NativeBinary,
}

impl std::fmt::Display for CommandDialect {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CommandDialect::Posix => write!(f, "POSIX"),
            CommandDialect::CmdBuiltin => write!(f, "CMD_BUILTIN"),
            CommandDialect::PowerShell => write!(f, "POWERSHELL"),
            CommandDialect::WindowsBatch => write!(f, "WINDOWS_BATCH"),
            CommandDialect::NativeBinary => write!(f, "NATIVE_BINARY"),
        }
    }
}

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
    pub dialect: CommandDialect,
    pub shell_compatibility: ShellCompatibility,
}

pub fn resolve_command(raw_program: &str, requested_shell: &str) -> CommandResolution {
    let clean_program = clean_unquoted_path(raw_program);
    let dialect = detect_command_dialect(&clean_program);

    // 1. Explicit shell request overrides auto-detection
    match requested_shell {
        "bash" => {
            return CommandResolution {
                original_command: raw_program.to_string(),
                resolved_executable: Some(clean_program.clone()),
                args_prefix: vec!["-c".to_string(), clean_program.clone()],
                resolution_source: "explicit(bash)".to_string(),
                recommended_shell: "bash".to_string(),
                dialect,
                shell_compatibility: ShellCompatibility {
                    cmd: false,
                    powershell: false,
                    bash: true,
                    notes: Some("Explicit execution via Bash (bash -c). Expects POSIX output.".to_string()),
                },
            };
        }
        "cmd" => {
            return CommandResolution {
                original_command: raw_program.to_string(),
                resolved_executable: Some("cmd.exe".to_string()),
                args_prefix: vec!["/c".to_string(), clean_program.clone()],
                resolution_source: "explicit(cmd)".to_string(),
                recommended_shell: "cmd".to_string(),
                dialect,
                shell_compatibility: ShellCompatibility {
                    cmd: true,
                    powershell: false,
                    bash: false,
                    notes: Some("Explicit execution via Windows Command Prompt (cmd.exe /c).".to_string()),
                },
            };
        }
        "powershell" => {
            return CommandResolution {
                original_command: raw_program.to_string(),
                resolved_executable: Some(clean_program.clone()),
                args_prefix: vec!["-NoProfile".to_string(), "-NonInteractive".to_string(), "-Command".to_string(), clean_program.clone()],
                resolution_source: "explicit(powershell)".to_string(),
                recommended_shell: "powershell".to_string(),
                dialect,
                shell_compatibility: ShellCompatibility {
                    cmd: false,
                    powershell: true,
                    bash: false,
                    notes: Some("Explicit execution via PowerShell (-Command). Expects PowerShell table/object output.".to_string()),
                },
            };
        }
        "direct" => {
            return CommandResolution {
                original_command: raw_program.to_string(),
                resolved_executable: Some(clean_program.clone()),
                args_prefix: vec![],
                resolution_source: "explicit(direct)".to_string(),
                recommended_shell: "direct".to_string(),
                dialect,
                shell_compatibility: ShellCompatibility {
                    cmd: true,
                    powershell: true,
                    bash: true,
                    notes: Some("Direct process spawning without intermediate shell.".to_string()),
                },
            };
        }
        _ => {}
    }

    // 2. AUTO-DETECTION MODE (by dialect and OS environment)

    // A. CMD Built-ins (dir, copy, type, del, etc.)
    if dialect == CommandDialect::CmdBuiltin {
        if cfg!(windows) {
            return CommandResolution {
                original_command: raw_program.to_string(),
                resolved_executable: Some("cmd.exe".to_string()),
                args_prefix: vec!["/c".to_string(), clean_program.clone()],
                resolution_source: "cmd_builtin".to_string(),
                recommended_shell: "cmd".to_string(),
                dialect,
                shell_compatibility: ShellCompatibility {
                    cmd: true,
                    powershell: false,
                    bash: false,
                    notes: Some(format!("'{}' is a Windows CMD internal command. Executed via cmd.exe /c.", clean_program)),
                },
            };
        } else {
            return CommandResolution {
                original_command: raw_program.to_string(),
                resolved_executable: None,
                args_prefix: vec![],
                resolution_source: "cmd_builtin_unsupported_on_linux".to_string(),
                recommended_shell: "bash".to_string(),
                dialect,
                shell_compatibility: ShellCompatibility {
                    cmd: false,
                    powershell: false,
                    bash: false,
                    notes: Some(format!("'{}' is a Windows CMD internal command and does not exist on Linux. Use POSIX equivalent (e.g. 'ls' for dir, 'cat' for type).", clean_program)),
                },
            };
        }
    }

    // B. POSIX Commands (ls, grep, cat, awk, sed, etc.)
    if dialect == CommandDialect::Posix {
        if cfg!(windows) {
            let bash_bin = find_windows_bash();
            return CommandResolution {
                original_command: raw_program.to_string(),
                resolved_executable: bash_bin.clone(),
                args_prefix: if bash_bin.is_some() { vec!["-c".to_string(), clean_program.clone()] } else { vec![] },
                resolution_source: if bash_bin.is_some() { "posix_bash".to_string() } else { "posix_requires_bash".to_string() },
                recommended_shell: if bash_bin.is_some() { "bash".to_string() } else { "posix_unavailable".to_string() },
                dialect,
                shell_compatibility: ShellCompatibility {
                    cmd: false,
                    powershell: false, // Do NOT alias to PowerShell, user explicitly wants POSIX output
                    bash: bash_bin.is_some(),
                    notes: Some(format!(
                        "'{}' is a POSIX/Linux command expecting Unix output. In Windows, it executes via Bash ('bash -c'). For native Windows listing, use 'dir' (CMD); for PowerShell table output, pass shell: 'powershell'.",
                        clean_program
                    )),
                },
            };
        } else {
            return CommandResolution {
                original_command: raw_program.to_string(),
                resolved_executable: Some(clean_program.clone()),
                args_prefix: vec!["-c".to_string(), clean_program.clone()],
                resolution_source: "posix_native".to_string(),
                recommended_shell: "bash".to_string(),
                dialect,
                shell_compatibility: ShellCompatibility {
                    cmd: false,
                    powershell: false,
                    bash: true,
                    notes: Some(format!("Native POSIX command '{}' running in Linux Bash.", clean_program)),
                },
            };
        }
    }

    // C. PowerShell Cmdlets (Get-Process, Test-Path, *.ps1)
    if dialect == CommandDialect::PowerShell {
        let shell_bin = if is_command_in_path("pwsh.exe") { "pwsh.exe" } else { "powershell.exe" };
        return CommandResolution {
            original_command: raw_program.to_string(),
            resolved_executable: Some(shell_bin.to_string()),
            args_prefix: vec!["-NoProfile".to_string(), "-NonInteractive".to_string(), "-Command".to_string(), clean_program.clone()],
            resolution_source: "powershell_cmdlet".to_string(),
            recommended_shell: "powershell".to_string(),
            dialect,
            shell_compatibility: ShellCompatibility {
                cmd: false,
                powershell: true,
                bash: false,
                notes: Some(format!("'{}' is a PowerShell cmdlet. Executed via PowerShell / pwsh.", clean_program)),
            },
        };
    }

    // D. Environment Variables for Program Mapping
    if let Some((path_str, var_name)) = find_in_env_variables(&clean_program) {
        let (shell, prefix) = determine_exec_wrapper(&path_str, &clean_program);
        return CommandResolution {
            original_command: raw_program.to_string(),
            resolved_executable: Some(path_str.clone()),
            args_prefix: prefix,
            resolution_source: format!("env_variable({})", var_name),
            recommended_shell: shell,
            dialect,
            shell_compatibility: analyze_compatibility(&path_str, &clean_program),
        };
    }

    // E. Standard PATH + PATHEXT
    if let Some((path_str, _ext_kind)) = search_path_with_pathext(&clean_program) {
        let (shell, prefix) = determine_exec_wrapper(&path_str, &clean_program);
        return CommandResolution {
            original_command: raw_program.to_string(),
            resolved_executable: Some(path_str.clone()),
            args_prefix: prefix,
            resolution_source: "pathext".to_string(),
            recommended_shell: shell,
            dialect,
            shell_compatibility: analyze_compatibility(&path_str, &clean_program),
        };
    }

    // F. System Directories on Windows (System32, Sysnative)
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
                dialect,
                shell_compatibility: analyze_compatibility(&path_str, &clean_program),
            };
        }
    }

    // G. Fallback: Not resolved on disk
    let fallback_prefix = if cfg!(windows) {
        vec!["/c".to_string(), clean_program.clone()]
    } else {
        vec!["-c".to_string(), clean_program.clone()]
    };

    CommandResolution {
        original_command: raw_program.to_string(),
        resolved_executable: None,
        args_prefix: fallback_prefix,
        resolution_source: "unknown".to_string(),
        recommended_shell: if cfg!(windows) { "cmd".to_string() } else { "bash".to_string() },
        dialect,
        shell_compatibility: ShellCompatibility {
            cmd: false,
            powershell: false,
            bash: false,
            notes: Some(format!("Program '{}' was not found in PATH, environment mappings, or system directories.", clean_program)),
        },
    }
}

pub fn detect_command_dialect(program: &str) -> CommandDialect {
    let clean = clean_unquoted_path(program);
    let lower = clean.to_lowercase();
    let first_word = lower.split_whitespace().next().unwrap_or(&lower);

    if is_cmd_builtin(first_word) {
        CommandDialect::CmdBuiltin
    } else if is_powershell_cmdlet(first_word) {
        CommandDialect::PowerShell
    } else if is_posix_command(first_word) || (!cfg!(windows) && is_shell_command_line(&clean)) {
        CommandDialect::Posix
    } else if lower.ends_with(".cmd") || lower.ends_with(".bat") {
        CommandDialect::WindowsBatch
    } else if lower.ends_with(".ps1") {
        CommandDialect::PowerShell
    } else if is_shell_command_line(&clean) {
        if cfg!(windows) {
            CommandDialect::CmdBuiltin
        } else {
            CommandDialect::Posix
        }
    } else {
        CommandDialect::NativeBinary
    }
}

pub fn is_shell_command_line(cmd: &str) -> bool {
    cmd.contains(';')
        || cmd.contains('|')
        || cmd.contains('&')
        || cmd.contains('>')
        || cmd.contains('<')
        || cmd.contains('$')
        || cmd.contains('\n')
        || cmd.contains('`')
        || cmd.contains('*')
}

pub fn clean_unquoted_path(path: &str) -> String {
    let trimmed = path.trim();
    if (trimmed.starts_with('"') && trimmed.ends_with('"'))
        || (trimmed.starts_with('\'') && trimmed.ends_with('\''))
    {
        if trimmed.len() >= 2 {
            return trimmed[1..trimmed.len() - 1].to_string();
        }
    }
    if trimmed.contains(r#":\""#) && trimmed.contains('"') {
        return trimmed.replace('"', "");
    }
    trimmed.to_string()
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
    if (trimmed.contains(r#"\""#) || trimmed.contains(r#"/"#) || trimmed.contains(r#":\"#)) && trimmed.contains('"') {
        let clean = trimmed.replace('"', "");
        if clean.contains(' ') {
            return format!("\"{}\"", clean);
        }
        return clean;
    }

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

fn is_posix_command(cmd: &str) -> bool {
    matches!(
        cmd,
        "ls" | "cat" | "grep" | "awk" | "sed" | "chmod" | "chown" | "export"
            | "source" | "uname" | "head" | "tail" | "touch" | "wc" | "cut"
            | "sort" | "uniq" | "tr" | "pwd" | "which" | "find" | "df"
            | "du" | "env" | "tar" | "gzip" | "gunzip" | "xargs"
    )
}

pub fn find_windows_bash() -> Option<String> {
    let git_bash = Path::new(r"C:\Program Files\Git\bin\bash.exe");
    if git_bash.is_file() {
        return Some(git_bash.to_string_lossy().to_string());
    }
    let msys_bash = Path::new(r"C:\msys64\usr\bin\bash.exe");
    if msys_bash.is_file() {
        return Some(msys_bash.to_string_lossy().to_string());
    }
    // Check if there is a bash in PATH that is NOT in System32 / Sysnative / SysWOW64
    if let Ok(path_var) = std::env::var("PATH") {
        for dir in path_var.split(';') {
            let lower = dir.to_lowercase();
            if lower.contains("system32") || lower.contains("sysnative") || lower.contains("syswow64") {
                continue;
            }
            let candidate = Path::new(dir).join("bash.exe");
            if candidate.is_file() {
                return Some(candidate.to_string_lossy().to_string());
            }
        }
    }
    None
}

fn find_in_env_variables(program: &str) -> Option<(String, String)> {
    let upper = program.to_uppercase();

    // Direct var matching PROGRAM
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

    // Home directory variants
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
        for (k, v) in std::env::vars() {
            for expected in keys {
                if k.eq_ignore_ascii_case(expected) {
                    return Some((v, k));
                }
            }
        }
    } else {
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
