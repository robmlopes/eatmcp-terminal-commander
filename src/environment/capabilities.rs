use serde::{Deserialize, Serialize};

use super::fingerprint::EnvironmentFingerprint;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityInfo {
    pub capability: String,
    pub description: String,
    pub supported: bool,
    pub implementation: String,
}

pub fn get_capabilities(fp: &EnvironmentFingerprint) -> Vec<CapabilityInfo> {
    let mut caps = Vec::new();

    // Environment capabilities
    caps.push(CapabilityInfo {
        capability: "environment.inspect".to_string(),
        description: "Returns deep environment fingerprint with cached detection".to_string(),
        supported: true,
        implementation: "native-engine".to_string(),
    });

    caps.push(CapabilityInfo {
        capability: "environment.capabilities".to_string(),
        description: "Returns all supported capabilities and underlying OS implementations".to_string(),
        supported: true,
        implementation: "native-engine".to_string(),
    });

    // Command execution capabilities
    caps.push(CapabilityInfo {
        capability: "command.validate".to_string(),
        description: "Validates commands against safety policies and classifies risk".to_string(),
        supported: true,
        implementation: "safety-classifier".to_string(),
    });

    caps.push(CapabilityInfo {
        capability: "command.execute".to_string(),
        description: "Direct process execution without intermediate shell by default".to_string(),
        supported: true,
        implementation: "direct-process-spawner".to_string(),
    });

    // Service capabilities
    let service_supported = fp.init_system != "unknown";
    let service_impl = match fp.init_system.as_str() {
        "windows-services" => "sc.exe / Windows Service Controller".to_string(),
        "systemd" => "systemctl".to_string(),
        "openrc" => "rc-service".to_string(),
        "termux-services" => "sv-enable / sv-down".to_string(),
        _ => "unsupported".to_string(),
    };

    for cap in &["service.status", "service.start", "service.stop", "service.restart"] {
        caps.push(CapabilityInfo {
            capability: cap.to_string(),
            description: format!("Manage system services via {}", service_impl),
            supported: service_supported,
            implementation: service_impl.clone(),
        });
    }

    // Package capabilities
    let pkg_supported = fp.package_manager != "none";
    let pkg_impl = format!("native package manager: {}", fp.package_manager);
    for cap in &["package.install", "package.remove"] {
        caps.push(CapabilityInfo {
            capability: cap.to_string(),
            description: format!("Manage software packages via {}", fp.package_manager),
            supported: pkg_supported,
            implementation: pkg_impl.clone(),
        });
    }

    // Process & System capabilities
    caps.push(CapabilityInfo {
        capability: "process.list".to_string(),
        description: "Inspect running processes with PID and memory stats".to_string(),
        supported: true,
        implementation: "sysinfo-native".to_string(),
    });

    caps.push(CapabilityInfo {
        capability: "network.port_check".to_string(),
        description: "Verify TCP port listening state via native non-blocking socket".to_string(),
        supported: true,
        implementation: "tokio-net-tcp".to_string(),
    });

    caps.push(CapabilityInfo {
        capability: "filesystem.exists".to_string(),
        description: "Verify file and directory presence and metadata".to_string(),
        supported: true,
        implementation: "tokio-fs".to_string(),
    });

    caps
}
