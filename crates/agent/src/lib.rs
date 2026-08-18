use chrono::Utc;
use helper::{HelperApi, HelperError};
use protocol::{Capability, Command, CommandResult, CommandStatus, OperationError};

#[derive(Debug, Clone)]
pub struct AgentRuntime {
    pub helper: HelperApi,
    pub capabilities: Vec<Capability>,
}

impl AgentRuntime {
    pub fn new(helper: HelperApi, capabilities: Vec<Capability>) -> Self {
        Self {
            helper,
            capabilities,
        }
    }

    pub async fn execute_command(&self, command: &Command) -> CommandResult {
        let required = command.command_type.required_capability();
        if !self.capabilities.contains(&required) {
            return CommandResult {
                command_id: command.id,
                status: CommandStatus::FAILED,
                finished_at: Utc::now(),
                error: Some(OperationError {
                    code: "CAPABILITY_UNAVAILABLE".to_string(),
                    message: format!("missing capability {}.{}", required.name, required.version),
                }),
            };
        }

        let result = match &command.command_type {
            protocol::CommandType::ServiceRestart { service } => self.helper.restart_service(service).await,
            protocol::CommandType::ServiceStatus { .. } => Ok(()),
        };

        match result {
            Ok(()) => CommandResult {
                command_id: command.id,
                status: CommandStatus::SUCCEEDED,
                finished_at: Utc::now(),
                error: None,
            },
            Err(e) => CommandResult {
                command_id: command.id,
                status: CommandStatus::FAILED,
                finished_at: Utc::now(),
                error: Some(map_helper_error(e)),
            },
        }
    }
}

fn map_helper_error(error: HelperError) -> OperationError {
    match error {
        HelperError::ServiceNotAllowlisted => OperationError {
            code: "PERMISSION_DENIED".to_string(),
            message: "service is not in helper allowlist".to_string(),
        },
        HelperError::InvalidServiceName => OperationError {
            code: "SERVICE_NOT_FOUND".to_string(),
            message: "invalid service name".to_string(),
        },
        HelperError::SystemCommandFailed(message) => OperationError {
            code: "SYSTEM_COMMAND_FAILED".to_string(),
            message,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use protocol::{CommandType, RiskLevel};
    use uuid::Uuid;

    #[tokio::test]
    async fn rejects_command_when_capability_is_missing() {
        let runtime = AgentRuntime::new(
            HelperApi::from_allowlist(vec!["nginx.service".to_string()]),
            vec![],
        );
        let command = Command {
            id: Uuid::new_v4(),
            server_id: Uuid::new_v4(),
            command_type: CommandType::ServiceRestart {
                service: "nginx.service".to_string(),
            },
            created_at: Utc::now(),
            expires_at: Utc::now(),
            status: CommandStatus::QUEUED,
            idempotency_key: "key".to_string(),
            risk_level: RiskLevel::MEDIUM,
        };

        let result = runtime.execute_command(&command).await;
        assert_eq!(result.status, CommandStatus::FAILED);
        assert_eq!(result.error.expect("error").code, "CAPABILITY_UNAVAILABLE");
    }
}
