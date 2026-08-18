use chrono::Utc;
use protocol::{
    BackupState, Capability, Command, CommandResult, CommandStatus, HelperRequest, HelperResponse,
    OperationError, SecurityState,
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
        .map(|_| ())
    }
    pub async fn start_service(&self, service: &str) -> Result<(), OperationError> {
        self.request(HelperRequest::StartService {
            service: service.into(),
        })
        .await
        .map(|_| ())
    }
    pub async fn stop_service(&self, service: &str) -> Result<(), OperationError> {
        self.request(HelperRequest::StopService {
            service: service.into(),
        })
        .await
        .map(|_| ())
    }
    pub async fn refresh_packages(&self) -> Result<(), OperationError> {
        self.request(HelperRequest::PackagesRefresh)
            .await
            .map(|_| ())
    }
    pub async fn upgrade_security_packages(&self) -> Result<(), OperationError> {
        self.request(HelperRequest::PackagesUpgradeSecurity)
            .await
            .map(|_| ())
    }
    pub async fn upgrade_all_packages(&self) -> Result<(), OperationError> {
        self.request(HelperRequest::PackagesUpgradeAll)
            .await
            .map(|_| ())
    }
    pub async fn reboot(&self) -> Result<(), OperationError> {
        self.request(HelperRequest::SystemReboot).await.map(|_| ())
    }
    pub async fn journal(&self, service: &str, lines: u32) -> Result<String, OperationError> {
        Ok(self
            .request(HelperRequest::Journal {
                service: service.into(),
                lines,
            })
            .await?
            .unwrap_or_default())
    }
    pub async fn docker_list(&self) -> Result<String, OperationError> {
        Ok(self
            .request(HelperRequest::DockerList)
            .await?
            .unwrap_or_default())
    }
    pub async fn docker_start(&self, container: &str) -> Result<(), OperationError> {
        self.request(HelperRequest::DockerStart {
            container: container.into(),
        })
        .await
        .map(|_| ())
    }
    pub async fn docker_stop(&self, container: &str) -> Result<(), OperationError> {
        self.request(HelperRequest::DockerStop {
            container: container.into(),
        })
        .await
        .map(|_| ())
    }
    pub async fn docker_restart(&self, container: &str) -> Result<(), OperationError> {
        self.request(HelperRequest::DockerRestart {
            container: container.into(),
        })
        .await
        .map(|_| ())
    }
    pub async fn docker_compose_start(&self, project: &str) -> Result<(), OperationError> {
        self.request(HelperRequest::DockerComposeStart {
            project: project.into(),
        })
        .await
        .map(|_| ())
    }
    pub async fn docker_compose_stop(&self, project: &str) -> Result<(), OperationError> {
        self.request(HelperRequest::DockerComposeStop {
            project: project.into(),
        })
        .await
        .map(|_| ())
    }
    pub async fn docker_compose_restart(&self, project: &str) -> Result<(), OperationError> {
        self.request(HelperRequest::DockerComposeRestart {
            project: project.into(),
        })
        .await
        .map(|_| ())
    }
    pub async fn security_inspect(&self) -> Result<SecurityState, OperationError> {
        let output = self
            .request(HelperRequest::SecurityInspect)
            .await?
            .ok_or_else(|| OperationError {
                code: "INVALID_RESPONSE".into(),
                message: "security inspection returned no data".into(),
            })?;
        serde_json::from_str(&output).map_err(internal)
    }
    pub async fn backup_list(&self) -> Result<BackupState, OperationError> {
        let output = self
            .request(HelperRequest::BackupList)
            .await?
            .ok_or_else(|| OperationError {
                code: "INVALID_RESPONSE".into(),
                message: "backup inventory returned no data".into(),
            })?;
        serde_json::from_str(&output).map_err(internal)
    }
    pub async fn backup_create(
        &self,
        backup_id: Uuid,
        profile: &str,
    ) -> Result<(), OperationError> {
        self.request(HelperRequest::BackupCreate {
            backup_id: backup_id.to_string(),
            profile: profile.into(),
        })
        .await
        .map(|_| ())
    }
    pub async fn backup_verify(&self, backup: &str) -> Result<(), OperationError> {
        self.request(HelperRequest::BackupVerify {
            backup: backup.into(),
        })
        .await
        .map(|_| ())
    }
    async fn request(&self, request: HelperRequest) -> Result<Option<String>, OperationError> {
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
            Ok(response.output)
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
        let result: Result<Option<String>, OperationError> = match &command.command_type {
            protocol::CommandType::ServiceRestart { service } => {
                self.helper.restart_service(service).await.map(|_| None)
            }
            protocol::CommandType::ServiceStart { service } => {
                self.helper.start_service(service).await.map(|_| None)
            }
            protocol::CommandType::ServiceStop { service } => {
                self.helper.stop_service(service).await.map(|_| None)
            }
            protocol::CommandType::ServiceStatus { .. } => Ok(None),
            protocol::CommandType::PackagesRefresh => {
                self.helper.refresh_packages().await.map(|_| None)
            }
            protocol::CommandType::PackagesUpgradeSecurity => {
                self.helper.upgrade_security_packages().await.map(|_| None)
            }
            protocol::CommandType::PackagesUpgradeAll => {
                self.helper.upgrade_all_packages().await.map(|_| None)
            }
            protocol::CommandType::SystemReboot => self.helper.reboot().await.map(|_| {
                Some(format!(
                    "{{\"reboot_requested_at_uptime\":{}}}",
                    system::current_uptime_seconds()
                ))
            }),
            protocol::CommandType::LogsJournal { service, lines } => {
                self.helper.journal(service, *lines).await.map(Some)
            }
            protocol::CommandType::DockerStart { container } => {
                self.helper.docker_start(container).await.map(|_| None)
            }
            protocol::CommandType::DockerStop { container } => {
                self.helper.docker_stop(container).await.map(|_| None)
            }
            protocol::CommandType::DockerRestart { container } => {
                self.helper.docker_restart(container).await.map(|_| None)
            }
            protocol::CommandType::DockerComposeStart { project } => {
                self.helper.docker_compose_start(project).await.map(|_| None)
            }
            protocol::CommandType::DockerComposeStop { project } => {
                self.helper.docker_compose_stop(project).await.map(|_| None)
            }
            protocol::CommandType::DockerComposeRestart { project } => self
                .helper
                .docker_compose_restart(project)
                .await
                .map(|_| None),
            protocol::CommandType::BackupCreate { profile } => self
                .helper
                .backup_create(command.id, profile)
                .await
                .map(|_| Some(format!("{}.tar.gz", command.id))),
            protocol::CommandType::BackupVerify { backup } => {
                self.helper.backup_verify(backup).await.map(|_| None)
            }
        };
        match result {
            Ok(output) => CommandResult {
                command_id: command.id,
                status: if matches!(command.command_type, protocol::CommandType::SystemReboot) {
                    CommandStatus::UNKNOWN
                } else {
                    CommandStatus::SUCCEEDED
                },
                finished_at: Utc::now(),
                error: None,
                output,
            },
            Err(error) => CommandResult {
                command_id: command.id,
                status: CommandStatus::FAILED,
                finished_at: Utc::now(),
                error: Some(error),
                output: None,
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
        output: None,
    }
}
