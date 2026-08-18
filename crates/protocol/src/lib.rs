use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

pub const PROTOCOL_VERSION: &str = "1.0";

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
    #[serde(rename = "service.status")]
    ServiceStatus { service: String },
}

impl CommandType {
    pub fn conflict_group(&self) -> &'static str {
        match self {
            CommandType::ServiceRestart { .. } => "service.mutate",
            CommandType::ServiceStatus { .. } => "service.read",
        }
    }

    pub fn required_capability(&self) -> Capability {
        match self {
            CommandType::ServiceRestart { .. } => Capability {
                name: "systemd".to_string(),
                version: "v1".to_string(),
            },
            CommandType::ServiceStatus { .. } => Capability {
                name: "system.metrics".to_string(),
                version: "v1".to_string(),
            },
        }
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
    pub captured_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ServiceState {
    pub name: String,
    pub status: String,
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
        Err(ProtocolError::UnsupportedVersion(protocol_version.to_string()))
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
            command_type: CommandType::ServiceRestart {
                service: "nginx.service".to_string(),
            },
            created_at: Utc::now(),
            expires_at: Utc::now(),
            status: CommandStatus::QUEUED,
            idempotency_key: "abc".to_string(),
            risk_level: RiskLevel::MEDIUM,
        };

        let serialized = serde_json::to_string(&command).expect("serialize command");
        let parsed: Command = serde_json::from_str(&serialized).expect("deserialize command");

        assert_eq!(parsed.command_type, command.command_type);
        assert_eq!(parsed.status, CommandStatus::QUEUED);
    }

    #[test]
    fn protocol_version_validation_rejects_unsupported_versions() {
        let error = validate_protocol_version("0.9").expect_err("must reject");
        assert!(matches!(error, ProtocolError::UnsupportedVersion(_)));
    }
}
