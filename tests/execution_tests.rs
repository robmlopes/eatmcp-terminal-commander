use eatmcp::execution::process::run_command;

#[tokio::test]
async fn test_direct_process_run() {
    let prog = if cfg!(windows) { "cmd.exe" } else { "echo" };
    let args = if cfg!(windows) { vec!["/c".to_string(), "echo".to_string(), "eatmcp_ok".to_string()] } else { vec!["eatmcp_ok".to_string()] };

    let output = run_command(prog, &args, None, 5000, false, "cmd")
        .await
        .expect("Process failed to run");

    assert_eq!(output.exit_code, Some(0));
    assert!(output.stdout.contains("eatmcp_ok"));
    assert!(!output.timed_out);
}

#[tokio::test]
async fn test_process_timeout() {
    // Run a sleep/ping command with a tiny timeout of 100ms
    let (prog, args) = if cfg!(windows) {
        ("ping.exe", vec!["-n".to_string(), "5".to_string(), "127.0.0.1".to_string()])
    } else {
        ("sleep", vec!["5".to_string()])
    };

    let output = run_command(prog, &args, None, 200, false, "cmd")
        .await
        .expect("Process failed to spawn");

    assert!(output.timed_out);
    assert_eq!(output.exit_code, None);
}

#[tokio::test]
async fn test_resolve_and_execute_batch_tool() {
    if cfg!(windows) {
        // "npm" should automatically resolve to npm.cmd via cmd.exe /c
        let res = eatmcp::execution::resolver::resolve_command("npm");
        assert_eq!(res.recommended_shell, "cmd");
        assert_eq!(res.args_prefix[0], "/c");
        assert!(res.args_prefix[1].to_lowercase().ends_with("npm.cmd"));

        // Running "npm" without use_shell should now succeed without failing with "program not found"
        let output = run_command("npm", &["--version".to_string()], None, 10000, false, "cmd")
            .await
            .expect("Failed to execute npm through auto-resolved batch execution");
        assert_eq!(output.exit_code, Some(0));
        assert!(!output.stdout.is_empty());
    }
}

#[tokio::test]
async fn test_resolve_telnet_and_system_tools() {
    if cfg!(windows) {
        let res = eatmcp::execution::resolver::resolve_command("telnet");
        assert!(res.resolved_executable.is_some());
        assert!(res.resolved_executable.unwrap().to_lowercase().contains("system32"));
        assert_eq!(res.recommended_shell, "cmd");
        assert!(res.shell_compatibility.cmd);
        assert!(!res.shell_compatibility.bash); // Documented: bash cannot run Windows console telnet without winpty
    }
}

#[tokio::test]
async fn test_resolve_cmd_builtins() {
    let res = eatmcp::execution::resolver::resolve_command("dir");
    assert_eq!(res.resolution_source, "cmd_builtin");
    assert_eq!(res.recommended_shell, "cmd");
    assert_eq!(res.args_prefix, vec!["/c".to_string(), "dir".to_string()]);
}

#[tokio::test]
async fn test_resolve_env_variable_mapping() {
    // Set a custom env var pointing to whoami.exe or notepad.exe
    if cfg!(windows) {
        std::env::set_var("CUSTOM_APP_PATH", "C:\\Windows\\System32\\notepad.exe");
        let res = eatmcp::execution::resolver::resolve_command("custom_app");
        assert!(res.resolved_executable.is_some());
        assert_eq!(res.resolution_source, "env_variable(CUSTOM_APP_PATH)");
    }
}

#[test]
fn test_path_with_spaces_quoting_normalization() {
    use eatmcp::execution::resolver::{normalize_quoted_command, normalize_quoted_arg, clean_unquoted_path};

    // Clean path
    assert_eq!(clean_unquoted_path(r#"C:\"Program Files"\nodejs\npm.cmd"#), r#"C:\Program Files\nodejs\npm.cmd"#);
    assert_eq!(clean_unquoted_path(r#""C:\Program Files\nodejs\npm.cmd""#), r#"C:\Program Files\nodejs\npm.cmd"#);

    // Entire path quoted, never just the folder
    assert_eq!(normalize_quoted_command(r#"C:\Program Files\nodejs\npm.cmd"#), r#""C:\Program Files\nodejs\npm.cmd""#);
    assert_eq!(normalize_quoted_command(r#"C:\"Program Files"\nodejs\npm.cmd"#), r#""C:\Program Files\nodejs\npm.cmd""#);
    assert_eq!(normalize_quoted_command("whoami"), "whoami");

    // Args normalization
    assert_eq!(normalize_quoted_arg(r#"C:\Program Files\My Project"#), r#""C:\Program Files\My Project""#);
    assert_eq!(normalize_quoted_arg(r#"C:\"Program Files"\My Project"#), r#""C:\Program Files\My Project""#);
    assert_eq!(normalize_quoted_arg("simple_arg"), "simple_arg");
}

#[tokio::test]
async fn test_run_command_with_spaces_in_arguments() {
    if cfg!(windows) {
        // Run dir on a directory with space ("C:\Program Files")
        let output = run_command("dir", &[r#"C:\Program Files"#.to_string()], None, 10000, false, "cmd")
            .await
            .expect("Failed to execute dir with spaces in path");
        assert_eq!(output.exit_code, Some(0));
        assert!(output.stdout.contains("Program Files"));
    }
}
