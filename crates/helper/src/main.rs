mod firewall;
mod restore_preflight;
mod restore_transaction;

use anyhow::Context;
use helper::{HelperApi, HelperError};
use protocol::{HelperRequest, HelperResponse, OperationError};
use std::{path::Path, sync::Arc};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::{UnixListener, UnixStream},
};
use tracing::{error, info};

fn map_error(error: HelperError) -> OperationError {
    match error {
        HelperError::ServiceNotAllowlisted => OperationError {
            code: "PERMISSION_DENIED".into(),
            message: "service is not allowlisted".into(),
        },
        HelperError::InvalidServiceName => OperationError {
            code: "SERVICE_NOT_FOUND".into(),
            message: "invalid service name".into(),
        },
        HelperError::InvalidContainerReference => OperationError {
            code: "CONTAINER_NOT_FOUND".into(),
            message: "invalid container reference".into(),
        },
        HelperError::InvalidComposeProject => OperationError {
            code: "STACK_NOT_FOUND".into(),
            message: "invalid Compose project name".into(),
        },
        HelperError::ComposeProjectNotFound => OperationError {
            code: "STACK_NOT_FOUND".into(),
            message: "Compose project was not discovered on this server".into(),
        },
        HelperError::InvalidBackupReference => OperationError {
            code: "BACKUP_NOT_FOUND".into(),
            message: "invalid backup reference".into(),
        },
        HelperError::BackupIntegrityFailed => OperationError {
            code: "BACKUP_INTEGRITY_FAILED".into(),
            message: "backup integrity verification failed".into(),
        },
        HelperError::InvalidRequest => OperationError {
            code: "INVALID_REQUEST".into(),
            message: "invalid helper request parameters".into(),
        },
        HelperError::UtilityUnavailable(name) => OperationError {
            code: "CAPABILITY_UNAVAILABLE".into(),
            message: format!("required utility unavailable: {name}"),
        },
        HelperError::SystemCommandFailed(message) => OperationError {
            code: "SYSTEM_COMMAND_FAILED".into(),
            message,
        },
    }
}

async fn execute_request(
    api: &HelperApi,
    request: HelperRequest,
) -> Result<Option<String>, HelperError> {
    match request {
        HelperRequest::RestartService { service } => {
            api.restart_service(&service).await.map(|_| None)
        }
        HelperRequest::StartService { service } => api.start_service(&service).await.map(|_| None),
        HelperRequest::StopService { service } => api.stop_service(&service).await.map(|_| None),
        HelperRequest::PackagesRefresh => api.refresh_packages().await.map(|_| None),
        HelperRequest::PackagesUpgradeSecurity => {
            api.upgrade_security_packages().await.map(|_| None)
        }
        HelperRequest::PackagesUpgradeAll => api.upgrade_all_packages().await.map(|_| None),
        HelperRequest::SystemReboot => api.reboot().await.map(|_| None),
        HelperRequest::Journal { service, lines } => api.journal(&service, lines).await.map(Some),
        HelperRequest::DockerList => api.docker_list().await.map(Some),
        HelperRequest::DockerStart { container } => {
            api.docker_start(&container).await.map(|_| None)
        }
        HelperRequest::DockerStop { container } => api.docker_stop(&container).await.map(|_| None),
        HelperRequest::DockerRestart { container } => {
            api.docker_restart(&container).await.map(|_| None)
        }
        HelperRequest::DockerComposeStart { project } => {
            api.docker_compose_start(&project).await.map(|_| None)
        }
        HelperRequest::DockerComposeStop { project } => {
            api.docker_compose_stop(&project).await.map(|_| None)
        }
        HelperRequest::DockerComposeRestart { project } => {
            api.docker_compose_restart(&project).await.map(|_| None)
        }
        HelperRequest::SecurityInspect => api.security_inspect().await.and_then(|state| {
            serde_json::to_string(&state)
                .map(Some)
                .map_err(|e| HelperError::SystemCommandFailed(e.to_string()))
        }),
        HelperRequest::SecurityFirewallEnable { rollback_id } => {
            firewall::enable(&rollback_id).await.map(|_| None)
        }
        HelperRequest::SecurityFirewallCommit { rollback_id } => {
            firewall::commit(&rollback_id).await.map(|_| None)
        }
        HelperRequest::BackupList => api.backup_list().await.and_then(|state| {
            serde_json::to_string(&state)
                .map(Some)
                .map_err(|e| HelperError::SystemCommandFailed(e.to_string()))
        }),
        HelperRequest::BackupCreate { backup_id, profile } => {
            api.backup_create(&backup_id, &profile).await.map(|_| None)
        }
        HelperRequest::BackupVerify { backup } => api.backup_verify(&backup).await.map(|_| None),
        HelperRequest::BackupRestorePreflight { restore_id, backup } => {
            restore_preflight::run(&restore_id, &backup).await.map(Some)
        }
        HelperRequest::BackupRestoreApply { restore_id, backup } => {
            restore_transaction::apply(&restore_id, &backup)
                .await
                .map(Some)
        }
        HelperRequest::BackupRestoreCommit { restore_id } => {
            restore_transaction::commit(&restore_id).await.map(|_| None)
        }
    }
}

async fn handle_client(stream: UnixStream, api: Arc<HelperApi>) -> anyhow::Result<()> {
    let (reader, mut writer) = stream.into_split();
    let mut lines = BufReader::new(reader).lines();
    while let Some(line) = lines.next_line().await? {
        let response = match serde_json::from_str::<HelperRequest>(&line) {
            Ok(request) => match execute_request(&api, request).await {
                Ok(output) => HelperResponse {
                    ok: true,
                    error: None,
                    output,
                },
                Err(error) => HelperResponse {
                    ok: false,
                    error: Some(map_error(error)),
                    output: None,
                },
            },
            Err(_) => HelperResponse {
                ok: false,
                error: Some(OperationError {
                    code: "INVALID_REQUEST".into(),
                    message: "invalid helper request".into(),
                }),
                output: None,
            },
        };
        writer
            .write_all(serde_json::to_string(&response)?.as_bytes())
            .await?;
        writer.write_all(b"\n").await?;
    }
    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = std::env::args().collect::<Vec<_>>();
    if args.len() == 3 && args[1] == "--restore-rollback" {
        restore_transaction::rollback(&args[2]).await?;
        return Ok(());
    }
    if args.len() != 1 {
        anyhow::bail!("unsupported argus-helper invocation");
    }

    tracing_subscriber::fmt::init();
    let socket =
        std::env::var("ARGUS_HELPER_SOCKET").unwrap_or_else(|_| "/run/argus/helper.sock".into());
    if let Some(parent) = Path::new(&socket).parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    let _ = tokio::fs::remove_file(&socket).await;
    let listener =
        UnixListener::bind(&socket).with_context(|| format!("bind helper socket {socket}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        tokio::fs::set_permissions(&socket, std::fs::Permissions::from_mode(0o660)).await?;
    }
    let api = Arc::new(HelperApi::from_env());
    info!(socket = %socket, "argus helper listening");
    loop {
        let (stream, _) = listener.accept().await?;
        let api = api.clone();
        tokio::spawn(async move {
            if let Err(error) = handle_client(stream, api).await {
                error!(%error, "helper client failed");
            }
        });
    }
}
