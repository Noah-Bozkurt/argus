use chrono::Utc;
use protocol::{
    Capability, Command, CommandResult, CommandStatus, HelperRequest, HelperResponse,
    OperationError,
};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::UnixStream,
};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    pub control_plane_url: String,
    pub server_id: Uuid,
    pub agent_id: Uuid,
    pub credential: Option<String>,
    pub helper_socket: PathBuf,
    pub managed_services: Vec<String>,
}

impl AgentConfig {
    pub async fn load(path: &Path) -> anyhow::Result<Self> {
        Ok(serde_json::from_slice(&tokio::fs::read(path).await?)?)
    }
    pub async fn save(&self, path: &Path) -> anyhow::Result<()> {
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        tokio::fs::write(path, serde_json::to_vec_pretty(self)?).await?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            tokio::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)).await?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct HelperClient {
    socket: PathBuf,
}
impl HelperClient {
    pub fn new(socket: PathBuf) -> Self {
        Self { socket }
    }
    pub async fn restart_service(&self, service: &str) -> Result<(), OperationError> {
        self.request(HelperRequest::RestartService {
            service: service.into(),
        })
        .await
    }
    pub async fn start_service(&self, service: &str) -> Result<(), OperationError> {
        self.request(HelperRequest::StartService {
            service: service.into(),
        })
        .await
    }
    pub async fn stop_service(&self, service: &str) -> Result<(), OperationError> {
        self.request(HelperRequest::StopService {
            service: service.into(),
        })
        .await
    }
    pub async fn refresh_packages(&self) -> Result<(), OperationError> {
        self.request(HelperRequest::PackagesRefresh).await
    }
    pub async fn upgrade_security_packages(&self) -> Result<(), OperationError> {
        self.request(HelperRequest::PackagesUpgradeSecurity).await
    }
    pub async fn upgrade_all_packages(&self) -> Result<(), OperationError> {
        self.request(HelperRequest::PackagesUpgradeAll).await
    }
    pub async fn reboot(&self) -> Result<(), OperationError> {
        self.request(HelperRequest::SystemReboot).await
    }

    async fn request(&self, request: HelperRequest) -> Result<(), OperationError> {
        let mut stream = UnixStream::connect(&self.socket)
            .await
            .map_err(|e| OperationError {
                code: "HELPER_UNAVAILABLE".into(),
                message: e.to_string(),
            })?;
        stream
            .write_all(
                serde_json::to_string(&request)
                    .map_err(internal)?
                    .as_bytes(),
            )
            .await
            .map_err(io_error)?;
        stream.write_all(b"\n").await.map_err(io_error)?;
        let mut line = String::new();
        BufReader::new(stream)
            .read_line(&mut line)
            .await
            .map_err(io_error)?;
        let response: HelperResponse = serde_json::from_str(&line).map_err(internal)?;
        if response.ok {
            Ok(())
        } else {
            Err(response.error.unwrap_or(OperationError {
                code: "HELPER_FAILED".into(),
                message: "helper operation failed".into(),
            }))
        }
    }
}
fn io_error(e: std::io::Error) -> OperationError {
    OperationError {
        code: "HELPER_UNAVAILABLE".into(),
        message: e.to_string(),
    }
}
fn internal(e: impl std::fmt::Display) -> OperationError {
    OperationError {
        code: "INTERNAL_ERROR".into(),
        message: e.to_string(),
    }
}

#[derive(Debug, Clone)]
pub struct AgentRuntime {
    pub helper: HelperClient,
    pub capabilities: Vec<Capability>,
}
impl AgentRuntime {
    pub fn new(helper: HelperClient, capabilities: Vec<Capability>) -> Self {
        Self {
            helper,
            capabilities,
        }
    }
    pub async fn execute_command(&self, command: &Command) -> CommandResult {
        if command.expires_at < Utc::now() {
            return failed(
                command.id,
                "COMMAND_EXPIRED",
                "command expired before execution",
            );
        }
        let required = command.command_type.required_capability();
        if !self.capabilities.contains(&required) {
            return failed(
                command.id,
                "CAPABILITY_UNAVAILABLE",
                &format!("missing capability {}.{}", required.name, required.version),
            );
        }
        let result = match &command.command_type {
            protocol::CommandType::ServiceRestart { service } => {
                self.helper.restart_service(service).await
            }
            protocol::CommandType::ServiceStart { service } => {
                self.helper.start_service(service).await
            }
            protocol::CommandType::ServiceStop { service } => {
                self.helper.stop_service(service).await
            }
            protocol::CommandType::ServiceStatus { .. } => Ok(()),
            protocol::CommandType::PackagesRefresh => self.helper.refresh_packages().await,
            protocol::CommandType::PackagesUpgradeSecurity => {
                self.helper.upgrade_security_packages().await
            }
            protocol::CommandType::PackagesUpgradeAll => self.helper.upgrade_all_packages().await,
            protocol::CommandType::SystemReboot => self.helper.reboot().await,
        };
        match result {
            Ok(()) => CommandResult {
                command_id: command.id,
                status: CommandStatus::SUCCEEDED,
                finished_at: Utc::now(),
                error: None,
            },
            Err(error) => CommandResult {
                command_id: command.id,
                status: CommandStatus::FAILED,
                finished_at: Utc::now(),
                error: Some(error),
            },
        }
    }
}
fn failed(command_id: Uuid, code: &str, message: &str) -> CommandResult {
    CommandResult {
        command_id,
        status: CommandStatus::FAILED,
        finished_at: Utc::now(),
        error: Some(OperationError {
            code: code.into(),
            message: message.into(),
        }),
    }
}
