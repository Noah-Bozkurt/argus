use agent::{AgentConfig, AgentRuntime, HelperClient};
use anyhow::{Context, Result};
use protocol::{
    AgentHandshake, Capability, Command, DiagnosticsState, EnrollmentRequest, EnrollmentResponse,
    HeartbeatRequest, PROTOCOL_VERSION, ServiceJournal,
};
use reqwest::{Client, StatusCode};
use std::{
    path::{Path, PathBuf},
    time::{Duration, Instant},
};
use tokio::time::sleep;
use tracing::{error, info, warn};
use uuid::Uuid;

const UPDATE_INVENTORY_INTERVAL: Duration = Duration::from_secs(300);
const DIAGNOSTICS_INTERVAL: Duration = Duration::from_secs(60);
const JOURNAL_LINES: u32 = 50;

fn capabilities() -> Vec<Capability> {
    vec![
        Capability {
            name: "systemd".into(),
            version: "v1".into(),
        },
        Capability {
            name: "system.metrics".into(),
            version: "v1".into(),
        },
        Capability {
            name: "apt".into(),
            version: "v1".into(),
        },
        Capability {
            name: "system.reboot".into(),
            version: "v1".into(),
        },
        Capability {
            name: "logs.journal".into(),
            version: "v1".into(),
        },
    ]
}
fn config_path() -> PathBuf {
    std::env::var_os("ARGUS_AGENT_CONFIG")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/etc/argus/agent.json"))
}

async fn bootstrap_config(path: &Path, client: &Client) -> Result<AgentConfig> {
    if path.exists() {
        return AgentConfig::load(path).await;
    }
    let control_plane_url = std::env::var("ARGUS_CONTROL_PLANE_URL")
        .context("ARGUS_CONTROL_PLANE_URL is required for enrollment")?;
    let token = std::env::var("ARGUS_ENROLLMENT_TOKEN")
        .context("ARGUS_ENROLLMENT_TOKEN is required for first enrollment")?;
    let server_id = std::env::var("ARGUS_SERVER_ID")
        .context("ARGUS_SERVER_ID is required for first enrollment")?
        .parse()?;
    let agent_id = Uuid::new_v4();
    let snapshot = system::collect_snapshot(server_id, env!("CARGO_PKG_VERSION").to_string());
    let handshake = AgentHandshake {
        agent_id,
        server_id,
        agent_version: env!("CARGO_PKG_VERSION").into(),
        protocol_version: PROTOCOL_VERSION.into(),
        hostname: snapshot.hostname,
        os: snapshot.os,
        architecture: snapshot.architecture,
        capabilities: capabilities(),
    };
    let enrolled: EnrollmentResponse = client
        .post(format!("{control_plane_url}/enrollment/complete"))
        .json(&EnrollmentRequest { token, handshake })
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    let config = AgentConfig {
        control_plane_url,
        server_id,
        agent_id,
        credential: Some(enrolled.credential),
        helper_socket: std::env::var_os("ARGUS_HELPER_SOCKET")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("/run/argus/helper.sock")),
        managed_services: std::env::var("ARGUS_MANAGED_SERVICES")
            .unwrap_or_else(|_| "nginx.service".into())
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .collect(),
    };
    config.save(path).await?;
    Ok(config)
}

async fn collect_diagnostics(runtime: &AgentRuntime, services: &[String]) -> DiagnosticsState {
    let mut diagnostics = system::diagnostics_state();
    for service in services {
        match runtime.helper.journal(service, JOURNAL_LINES).await {
            Ok(output) => diagnostics.journals.push(ServiceJournal {
                service: service.clone(),
                output,
            }),
            Err(error) => {
                warn!(service=%service, code=%error.code, "failed to collect service journal")
            }
        }
    }
    diagnostics
}

async fn heartbeat(
    client: &Client,
    config: &AgentConfig,
    updates: &protocol::UpdateState,
    diagnostics: &DiagnosticsState,
) -> Result<()> {
    let mut snapshot =
        system::collect_snapshot(config.server_id, env!("CARGO_PKG_VERSION").to_string());
    snapshot.updates = updates.clone();
    snapshot.diagnostics = diagnostics.clone();
    let services = system::service_statuses(&config.managed_services)?;
    client
        .post(format!("{}/agent/heartbeat", config.control_plane_url))
        .bearer_auth(
            config
                .credential
                .as_deref()
                .context("agent credential missing")?,
        )
        .json(&HeartbeatRequest { snapshot, services })
        .send()
        .await?
        .error_for_status()?;
    Ok(())
}
async fn next_command(client: &Client, config: &AgentConfig) -> Result<Option<Command>> {
    let response = client
        .post(format!("{}/agent/commands/next", config.control_plane_url))
        .bearer_auth(
            config
                .credential
                .as_deref()
                .context("agent credential missing")?,
        )
        .send()
        .await?;
    if response.status() == StatusCode::NO_CONTENT {
        return Ok(None);
    }
    Ok(response.error_for_status()?.json().await?)
}
async fn submit_result(
    client: &Client,
    config: &AgentConfig,
    result: &protocol::CommandResult,
) -> Result<()> {
    client
        .post(format!(
            "{}/agent/commands/result",
            config.control_plane_url
        ))
        .bearer_auth(
            config
                .credential
                .as_deref()
                .context("agent credential missing")?,
        )
        .json(result)
        .send()
        .await?
        .error_for_status()?;
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();
    let client = Client::builder().timeout(Duration::from_secs(15)).build()?;
    let path = config_path();
    let config = bootstrap_config(&path, &client).await?;
    let runtime = AgentRuntime::new(
        HelperClient::new(config.helper_socket.clone()),
        capabilities(),
    );
    let mut backoff = 1_u64;
    let mut updates = system::update_state();
    let mut diagnostics = collect_diagnostics(&runtime, &config.managed_services).await;
    let mut next_update_inventory = Instant::now() + UPDATE_INVENTORY_INTERVAL;
    let mut next_diagnostics = Instant::now() + DIAGNOSTICS_INTERVAL;
    info!(server_id=%config.server_id, agent_id=%config.agent_id, "argus agent started");
    loop {
        if Instant::now() >= next_update_inventory {
            updates = system::update_state();
            next_update_inventory = Instant::now() + UPDATE_INVENTORY_INTERVAL;
        }
        if Instant::now() >= next_diagnostics {
            diagnostics = collect_diagnostics(&runtime, &config.managed_services).await;
            next_diagnostics = Instant::now() + DIAGNOSTICS_INTERVAL;
        }
        let cycle = async {
            heartbeat(&client, &config, &updates, &diagnostics).await?;
            if let Some(command) = next_command(&client, &config).await? {
                let command_id = command.id;
                let result = runtime.execute_command(&command).await;
                if let Err(error) = submit_result(&client, &config, &result).await {
                    warn!(%command_id, %error, "command executed but result submission failed; control plane will reconcile as unknown");
                    return Err(error);
                }
                if matches!(
                    command.command_type,
                    protocol::CommandType::PackagesRefresh
                        | protocol::CommandType::PackagesUpgradeSecurity
                        | protocol::CommandType::PackagesUpgradeAll
                ) {
                    updates = system::update_state();
                }
            }
            Result::<()>::Ok(())
        }
        .await;
        match cycle {
            Ok(()) => {
                backoff = 1;
                sleep(Duration::from_secs(5)).await;
            }
            Err(error) => {
                error!(%error, retry_seconds=backoff, "agent cycle failed");
                sleep(Duration::from_secs(backoff)).await;
                backoff = (backoff * 2).min(60);
            }
        }
    }
}
