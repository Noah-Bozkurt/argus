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
