use std::fs;
use std::path::{Path, PathBuf};

pub fn show_configs(host: &str, port: u16) {
    let exe_path = std::env::current_exe()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| "eatmcp".to_string());

    let args_slice = build_args_slice(host, port);
    let args_json = serde_json::to_string(&args_slice).unwrap_or_default();
    let args_cli = args_slice.join(" ");
    let args_yaml = if host == "127.0.0.1" {
        format!("args: ['--dev-port', '{}']", port)
    } else {
        format!("args: ['--dev-host', '{}', '--dev-port', '{}']", host, port)
    };

    println!("============================================================");
    println!("  EATMCP (Environment-Aware Terminal MCP) Configurations");
    println!("  Target: Host = {}, Port = {}", host, port);
    println!("============================================================");
    println!();
    println!("1. Claude Desktop (claude_desktop_config.json):");
    println!(r#"{{
  "mcpServers": {{
    "eatmcp": {{
      "command": "{}",
      "args": {}
    }}
  }}
}}"#, escape_json_path(&exe_path), args_json);

    println!();
    println!("2. Cursor (.cursor/mcp.json):");
    println!(r#"{{
  "mcpServers": {{
    "eatmcp": {{
      "command": "{}",
      "args": {}
    }}
  }}
}}"#, escape_json_path(&exe_path), args_json);

    println!();
    println!("3. Claude Code CLI:");
    println!(r#"claude mcp add eatmcp -- "{}" {}"#, exe_path, args_cli);

    println!();
    println!("4. Windsurf (~/.codeium/windsurf/mcp_config.json):");
    println!(r#"{{
  "mcpServers": {{
    "eatmcp": {{
      "command": "{}",
      "args": {}
    }}
  }}
}}"#, escape_json_path(&exe_path), args_json);

    println!();
    println!("5. DeepSeek Harness (cordis.yml / agent.cordis.yml):");
    println!(r#"- id: mcp-eatmcp
  name: '@deepseek-ai/dsh-mcp-client'
  config:
    transport: stdio
    serverName: eatmcp
    command: '{}'
    {}"#, exe_path, args_yaml);
    println!("============================================================");
}

pub fn install_for(target: &str, host: &str, port: u16) -> Result<(), Box<dyn std::error::Error>> {
    let exe_path = std::env::current_exe()?
        .to_string_lossy()
        .to_string();

    let args = build_args_slice(host, port);
    let str_args: Vec<&str> = args.iter().map(|s| s.as_str()).collect();

    match target.to_lowercase().as_str() {
        "claude" | "claude-desktop" => install_claude_desktop(&exe_path, &str_args),
        "cursor" => install_cursor(&exe_path, &str_args),
        "windsurf" => install_windsurf(&exe_path, &str_args),
        "all" => {
            println!("Detecting and installing EATMCP into all available harnesses (host={}, port={})...", host, port);
            let _ = install_claude_desktop(&exe_path, &str_args);
            let _ = install_cursor(&exe_path, &str_args);
            let _ = install_windsurf(&exe_path, &str_args);
            Ok(())
        }
        _ => {
            eprintln!("Unknown target '{}'. Available targets: claude-desktop, cursor, windsurf, all", target);
            Ok(())
        }
    }
}

fn build_args_slice(host: &str, port: u16) -> Vec<String> {
    let mut args = Vec::new();
    if host != "127.0.0.1" && !host.is_empty() {
        args.push("--dev-host".to_string());
        args.push(host.to_string());
    }
    args.push("--dev-port".to_string());
    args.push(port.to_string());
    args
}

fn install_claude_desktop(exe_path: &str, args: &[&str]) -> Result<(), Box<dyn std::error::Error>> {
    let path = if cfg!(windows) {
        std::env::var("APPDATA")
            .map(|a| PathBuf::from(a).join("Claude").join("claude_desktop_config.json"))
            .ok()
    } else if cfg!(target_os = "macos") {
        std::env::var("HOME")
            .map(|h| PathBuf::from(h).join("Library").join("Application Support").join("Claude").join("claude_desktop_config.json"))
            .ok()
    } else {
        std::env::var("HOME")
            .map(|h| PathBuf::from(h).join(".config").join("Claude").join("claude_desktop_config.json"))
            .ok()
    };

    if let Some(config_path) = path {
        inject_mcp_config(&config_path, "eatmcp", exe_path, args, "Claude Desktop")?;
    } else {
        println!("[-] Claude Desktop config path could not be resolved.");
    }
    Ok(())
}

fn install_cursor(exe_path: &str, args: &[&str]) -> Result<(), Box<dyn std::error::Error>> {
    let local_cursor = PathBuf::from(".cursor").join("mcp.json");
    if local_cursor.parent().map(|p| p.exists()).unwrap_or(false) {
        inject_mcp_config(&local_cursor, "eatmcp", exe_path, args, "Cursor (Project)")?;
        return Ok(());
    }

    if cfg!(windows) {
        if let Ok(appdata) = std::env::var("APPDATA") {
            let cursor_global = PathBuf::from(appdata)
                .join("Cursor")
                .join("User")
                .join("globalStorage")
                .join("rooveterinaryinc.cursor-mcp")
                .join("mcp.json");
            if cursor_global.parent().map(|p| p.exists()).unwrap_or(false) {
                inject_mcp_config(&cursor_global, "eatmcp", exe_path, args, "Cursor (Global)")?;
                return Ok(());
            }
        }
    }

    println!("[*] Cursor configuration: create '.cursor/mcp.json' in your project root.");
    Ok(())
}

fn install_windsurf(exe_path: &str, args: &[&str]) -> Result<(), Box<dyn std::error::Error>> {
    if let Ok(home) = std::env::var("USERPROFILE").or_else(|_| std::env::var("HOME")) {
        let p = PathBuf::from(home).join(".codeium").join("windsurf").join("mcp_config.json");
        if p.parent().map(|d| d.exists()).unwrap_or(false) {
            inject_mcp_config(&p, "eatmcp", exe_path, args, "Windsurf")?;
            return Ok(());
        }
    }
    println!("[-] Windsurf installation directory not found.");
    Ok(())
}

fn inject_mcp_config(
    file_path: &Path,
    server_name: &str,
    exe_path: &str,
    args: &[&str],
    label: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(parent) = file_path.parent() {
        fs::create_dir_all(parent)?;
    }

    let mut root: serde_json::Value = if file_path.exists() {
        let content = fs::read_to_string(file_path)?;
        serde_json::from_str(&content).unwrap_or_else(|_| serde_json::json!({}))
    } else {
        serde_json::json!({})
    };

    if !root.is_object() {
        root = serde_json::json!({});
    }

    let servers = root
        .as_object_mut()
        .unwrap()
        .entry("mcpServers")
        .or_insert_with(|| serde_json::json!({}));

    if let Some(server_map) = servers.as_object_mut() {
        server_map.insert(
            server_name.to_string(),
            serde_json::json!({
                "command": exe_path,
                "args": args
            }),
        );
        let updated = serde_json::to_string_pretty(&root)?;
        fs::write(file_path, updated)?;
        println!("[+] Successfully configured EATMCP in {} at: {:?}", label, file_path);
    }

    Ok(())
}

fn escape_json_path(path: &str) -> String {
    path.replace('\\', "\\\\")
}
