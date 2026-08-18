use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

pub const PROTOCOL_VERSION: &str = "1.2";

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
    fn command_round_trip_serialization_works() {
        let command = Command {
            id: Uuid::new_v4(),
            server_id: Uuid::new_v4(),
            command_type: CommandType::LogsJournal {
                service: "nginx.service".into(),
                lines: 100,
            },
            created_at: Utc::now(),
            expires_at: Utc::now(),
            status: CommandStatus::QUEUED,
            idempotency_key: "abc".into(),
            risk_level: RiskLevel::LOW,
        };
        let parsed: Command =
            serde_json::from_str(&serde_json::to_string(&command).unwrap()).unwrap();
        assert_eq!(parsed.command_type, command.command_type);
    }
    #[test]
    fn capabilities_are_explicit() {
        assert_eq!(
            CommandType::PackagesRefresh.required_capability().name,
            "apt"
        );
        assert_eq!(
            CommandType::SystemReboot.required_capability().name,
            "system.reboot"
        );
        assert_eq!(
            CommandType::LogsJournal {
                service: "nginx.service".into(),
                lines: 100,
            }
            .required_capability()
            .name,
            "logs.journal"
        );
    }
    #[test]
    fn disruptive_operations_require_maintenance() {
        assert!(!CommandType::PackagesRefresh.requires_maintenance());
        assert!(CommandType::PackagesUpgradeAll.requires_maintenance());
        assert!(CommandType::SystemReboot.requires_maintenance());
        assert!(
            !CommandType::LogsJournal {
                service: "nginx.service".into(),
                lines: 100,
            }
            .requires_maintenance()
        );
    }
    #[test]
    fn protocol_version_validation_rejects_unsupported_versions() {
        assert!(validate_protocol_version("1.1").is_err());
    }
}
