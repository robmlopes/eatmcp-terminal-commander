use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvironmentFingerprint {
    pub platform: String,
    pub distribution: String,
    pub version: String,
    pub kernel: String,
    pub architecture: String,
    pub shell: String,
    pub available_shells: Vec<String>,
    pub init_system: String,
    pub package_manager: String,
    pub available_package_managers: Vec<String>,
    pub user: String,
    pub is_admin: bool,
    pub is_container: bool,
    pub docker_available: bool,
    pub hostname: String,
    pub timestamp: u64,
    #[serde(default)]
    pub cache_age_ms: u64,
}
