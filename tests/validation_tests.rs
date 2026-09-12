use eatmcp::validation::checks::{validate_filesystem_path, validate_tcp_port, ValidationStatus};

#[tokio::test]
async fn test_validate_filesystem_exists() {
    let res = validate_filesystem_path("Cargo.toml").await;
    assert_eq!(res.status, ValidationStatus::Success);
    assert!(res.message.contains("exists"));

    let non_existent = validate_filesystem_path("non_existent_file_xyz_123.txt").await;
    assert_eq!(non_existent.status, ValidationStatus::Failed);
}

#[tokio::test]
async fn test_validate_tcp_port_closed() {
    // Port 59999 should normally be closed
    let res = validate_tcp_port("127.0.0.1", 59999, 500).await;
    assert_eq!(res.status, ValidationStatus::Failed);
}
