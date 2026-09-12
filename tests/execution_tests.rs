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
