use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

pub const PROTOCOL_VERSION: &str = "1.6";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentHandshake {
    pub agent_id: Uuid,
    pub server_id: Uuid,
    pub agent_version: String,
    pub protocol_version: String,
    pub hostname: String,
    pub os: String,
    pub architecture: String,
    pub capabilities: Vec<Capability>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct Capability {
    pub name: String,
    pub version: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum RiskLevel {
    LOW,
    MEDIUM,
    HIGH,
    CRITICAL,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum CommandStatus {
    QUEUED,
    ACCEPTED,
    RUNNING,
    SUCCEEDED,
    FAILED,
    UNKNOWN,
    EXPIRED,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind")]
pub enum CommandType {
    #[serde(rename = "service.restart")]
    ServiceRestart { service: String },
    #[serde(rename = "service.start")]
    ServiceStart { service: String },
    #[serde(rename = "service.stop")]
    ServiceStop { service: String },
    #[serde(rename = "service.status")]
    ServiceStatus { service: String },
    #[serde(rename = "packages.refresh")]
    PackagesRefresh,
    #[serde(rename = "packages.upgrade.security")]
    PackagesUpgradeSecurity,
    #[serde(rename = "packages.upgrade.all")]
    PackagesUpgradeAll,
    #[serde(rename = "system.reboot")]
    SystemReboot,
    #[serde(rename = "logs.journal")]
    LogsJournal { service: String, lines: u32 },
    #[serde(rename = "docker.start")]
    DockerStart { container: String },
    #[serde(rename = "docker.stop")]
    DockerStop { container: String },
    #[serde(rename = "docker.restart")]
    DockerRestart { container: String },
    #[serde(rename = "docker.compose.start")]
    DockerComposeStart { project: String },
    #[serde(rename = "docker.compose.stop")]
    DockerComposeStop { project: String },
    #[serde(rename = "docker.compose.restart")]
    DockerComposeRestart { project: String },
    #[serde(rename = "backup.create")]
    BackupCreate { profile: String },
    #[serde(rename = "backup.verify")]
    BackupVerify { backup: String },
}
impl CommandType {
    pub fn conflict_group(&self) -> &'static str {
        match self {
            CommandType::ServiceRestart { .. }
            | CommandType::ServiceStart { .. }
            | CommandType::ServiceStop { .. } => "service.mutate",
            CommandType::ServiceStatus { .. } => "service.read",
            CommandType::PackagesRefresh
            | CommandType::PackagesUpgradeSecurity
            | CommandType::PackagesUpgradeAll => "packages.mutate",
            CommandType::SystemReboot => "system.reboot",
            CommandType::LogsJournal { .. } => "logs.read",
            CommandType::DockerStart { .. }
            | CommandType::DockerStop { .. }
            | CommandType::DockerRestart { .. }
            | CommandType::DockerComposeStart { .. }
            | CommandType::DockerComposeStop { .. }
            | CommandType::DockerComposeRestart { .. } => "docker.mutate",
            CommandType::BackupCreate { .. } => "backup.create",
            CommandType::BackupVerify { .. } => "backup.verify",
        }
    }
    pub fn required_capability(&self) -> Capability {
        match self {
            CommandType::ServiceRestart { .. }
            | CommandType::ServiceStart { .. }
            | CommandType::ServiceStop { .. }
            | CommandType::ServiceStatus { .. } => Capability {
                name: "systemd".into(),
                version: "v1".into(),
            },
            CommandType::PackagesRefresh
            | CommandType::PackagesUpgradeSecurity
            | CommandType::PackagesUpgradeAll => Capability {
                name: "apt".into(),
                version: "v1".into(),
            },
            CommandType::SystemReboot => Capability {
                name: "system.reboot".into(),
                version: "v1".into(),
            },
            CommandType::LogsJournal { .. } => Capability {
                name: "logs.journal".into(),
                version: "v1".into(),
            },
            CommandType::DockerStart { .. }
            | CommandType::DockerStop { .. }
            | CommandType::DockerRestart { .. } => Capability {
                name: "docker".into(),
                version: "v1".into(),
            },
            CommandType::DockerComposeStart { .. }
            | CommandType::DockerComposeStop { .. }
            | CommandType::DockerComposeRestart { .. } => Capability {
                name: "docker.compose".into(),
                version: "v1".into(),
            },
            CommandType::BackupCreate { .. } | CommandType::BackupVerify { .. } => Capability {
                name: "backup".into(),
                version: "v1".into(),
            },
        }
    }
    pub fn requires_maintenance(&self) -> bool {
        matches!(
            self,
            CommandType::PackagesUpgradeSecurity
                | CommandType::PackagesUpgradeAll
                | CommandType::SystemReboot
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Command {
    pub id: Uuid,
    pub server_id: Uuid,
    pub command_type: CommandType,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub status: CommandStatus,
    pub idempotency_key: String,
    pub risk_level: RiskLevel,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CommandRequest {
    pub server_id: Uuid,
    pub command_type: CommandType,
    pub ttl_seconds: i64,
    pub idempotency_key: String,
    pub risk_level: RiskLevel,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OperationError {
    pub code: String,
    pub message: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CommandResult {
    pub command_id: Uuid,
    pub status: CommandStatus,
    pub finished_at: DateTime<Utc>,
    pub error: Option<OperationError>,
    #[serde(default)]
    pub output: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct UpdateState {
    pub supported: bool,
    pub pending_updates: u32,
    pub reboot_required: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ServiceJournal {
    pub service: String,
    pub output: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct DiagnosticsState {
    pub failed_units: Vec<String>,
    pub listening_tcp_ports: Vec<u16>,
    #[serde(default)]
    pub journals: Vec<ServiceJournal>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DockerContainer {
    pub id: String,
    pub name: String,
    pub image: String,
    pub state: String,
    pub status: String,
    pub ports: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct DockerState {
    pub available: bool,
    pub containers: Vec<DockerContainer>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SecurityFinding {
    pub severity: String,
    pub code: String,
    pub message: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct SecurityState {
    pub available: bool,
    pub firewall_status: String,
    pub firewall_rules: Vec<String>,
    pub ssh_password_auth: Option<bool>,
    pub ssh_root_login: String,
    pub automatic_security_updates: bool,
    pub findings: Vec<SecurityFinding>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BackupArtifact {
    pub name: String,
    pub profile: String,
    pub size_bytes: u64,
    pub created_unix: u64,
    pub sha256: String,
    pub verified: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct BackupState {
    pub available: bool,
    pub target: String,
    pub artifacts: Vec<BackupArtifact>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SystemSnapshot {
    pub server_id: Uuid,
    pub hostname: String,
    pub os: String,
    pub kernel: String,
    pub architecture: String,
    pub cpu_percent: f32,
    pub ram_percent: f32,
    pub disk_percent: f32,
    pub load: f64,
    pub uptime_seconds: u64,
    pub agent_version: String,
    #[serde(default)]
    pub updates: UpdateState,
    #[serde(default)]
    pub diagnostics: DiagnosticsState,
    #[serde(default)]
    pub docker: DockerState,
    #[serde(default)]
    pub security: SecurityState,
    #[serde(default)]
    pub backups: BackupState,
    pub captured_at: DateTime<Utc>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ServiceState {
    pub name: String,
    pub status: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnrollmentRequest {
    pub token: String,
    pub handshake: AgentHandshake,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnrollmentResponse {
    pub credential: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeartbeatRequest {
    pub snapshot: SystemSnapshot,
    pub services: Vec<ServiceState>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum HelperRequest {
    #[serde(rename = "service.restart")]
    RestartService { service: String },
    #[serde(rename = "service.start")]
    StartService { service: String },
    #[serde(rename = "service.stop")]
    StopService { service: String },
    #[serde(rename = "packages.refresh")]
    PackagesRefresh,
    #[serde(rename = "packages.upgrade.security")]
    PackagesUpgradeSecurity,
    #[serde(rename = "packages.upgrade.all")]
    PackagesUpgradeAll,
    #[serde(rename = "system.reboot")]
    SystemReboot,
    #[serde(rename = "logs.journal")]
    Journal { service: String, lines: u32 },
    #[serde(rename = "docker.list")]
    DockerList,
    #[serde(rename = "docker.start")]
    DockerStart { container: String },
    #[serde(rename = "docker.stop")]
    DockerStop { container: String },
    #[serde(rename = "docker.restart")]
    DockerRestart { container: String },
    #[serde(rename = "docker.compose.start")]
    DockerComposeStart { project: String },
    #[serde(rename = "docker.compose.stop")]
    DockerComposeStop { project: String },
    #[serde(rename = "docker.compose.restart")]
    DockerComposeRestart { project: String },
    #[serde(rename = "security.inspect")]
    SecurityInspect,
    #[serde(rename = "backup.list")]
    BackupList,
    #[serde(rename = "backup.create")]
    BackupCreate { backup_id: String, profile: String },
    #[serde(rename = "backup.verify")]
    BackupVerify { backup: String },
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HelperResponse {
    pub ok: bool,
    pub error: Option<OperationError>,
    #[serde(default)]
    pub output: Option<String>,
}
#[derive(Debug, Error)]
pub enum ProtocolError {
    #[error("unsupported protocol version: {0}")]
    UnsupportedVersion(String),
}
pub fn validate_protocol_version(protocol_version: &str) -> Result<(), ProtocolError> {
    if protocol_version == PROTOCOL_VERSION {
        Ok(())
    } else {
        Err(ProtocolError::UnsupportedVersion(
            protocol_version.to_string(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn backup_commands_require_backup_capability() {
        let create = CommandType::BackupCreate {
            profile: "system-config".into(),
        };
        let verify = CommandType::BackupVerify {
            backup: "abc.tar.gz".into(),
        };
        assert_eq!(create.required_capability().name, "backup");
        assert_eq!(verify.required_capability().name, "backup");
        assert!(!create.requires_maintenance());
    }
    #[test]
    fn compose_commands_require_compose_capability() {
        let command = CommandType::DockerComposeRestart {
            project: "app".into(),
        };
        assert_eq!(command.required_capability().name, "docker.compose");
        assert_eq!(command.conflict_group(), "docker.mutate");
    }
    #[test]
    fn security_state_defaults_safe_for_backward_compatibility() {
        let state = SecurityState::default();
        assert!(!state.available);
        assert!(state.findings.is_empty());
    }
    #[test]
    fn protocol_version_validation_rejects_older_versions() {
        assert!(validate_protocol_version("1.5").is_err());
    }
}
