use eatmcp::safety::classifier::classify_command;
use eatmcp::safety::policy::{PolicyAction, RiskLevel};

#[test]
fn test_classify_safe_commands() {
    let whoami_res = classify_command("whoami", &[]);
    assert_eq!(whoami_res.risk_level, RiskLevel::Low);
    assert_eq!(whoami_res.action, PolicyAction::Allow);

    let ping_res = classify_command("ping", &["127.0.0.1".to_string()]);
    assert_eq!(ping_res.risk_level, RiskLevel::Low);
    assert_eq!(ping_res.action, PolicyAction::Allow);
}

#[test]
fn test_classify_critical_deny() {
    let rm_root = classify_command("rm", &["-rf".to_string(), "/".to_string()]);
    assert_eq!(rm_root.risk_level, RiskLevel::Critical);
    assert_eq!(rm_root.action, PolicyAction::Deny);

    let format_disk = classify_command("format.com", &["C:".to_string()]);
    assert_eq!(format_disk.risk_level, RiskLevel::Critical);
    assert_eq!(format_disk.action, PolicyAction::Deny);

    let del_root = classify_command("del", &["/s".to_string(), "/q".to_string(), "C:\\".to_string()]);
    assert_eq!(del_root.risk_level, RiskLevel::Critical);
    assert_eq!(del_root.action, PolicyAction::Deny);
}

#[test]
fn test_classify_high_confirm() {
    let kill_f = classify_command("taskkill", &["/f".to_string(), "/im".to_string(), "notepad.exe".to_string()]);
    assert_eq!(kill_f.risk_level, RiskLevel::High);
    assert_eq!(kill_f.action, PolicyAction::Confirm);
}
