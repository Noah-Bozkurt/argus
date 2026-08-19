use agent::AgentConfig;
use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use serde_json::json;
use std::{
    path::{Path, PathBuf},
    process::Stdio,
    time::Duration,
};
use tokio::{io::AsyncWriteExt, net::UnixStream, process::Command};
use uuid::Uuid;

const FIRST_SERVER_SMOKE: &str = include_str!("../../../scripts/first-server-smoke.sh");
const FIRST_SERVER_UPDATE: &str = include_str!("../../../scripts/update-first-test.sh");
const INTERRUPTED_UPDATE_RECOVERY: &str =
    include_str!("../../../scripts/recover-interrupted-update.sh");

#[derive(Debug, Parser)]
#[command(name = "argusctl", about = "Argus local diagnostics and lifecycle CLI")]
struct Cli {
    #[arg(
        long,
        env = "ARGUS_AGENT_CONFIG",
        default_value = "/var/lib/argus/agent.json"
    )]
    config: PathBuf,
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    Status,
    Health,
    Connection,
    Smoke,
    Update {
        #[arg(long, default_value = "main")]
        version: String,
    },
    #[command(hide = true)]
    RecoverUpdate,
    System {
        #[command(subcommand)]
        command: SystemCommands,
    },
    Version,
}

#[derive(Debug, Subcommand)]
enum SystemCommands {
    Info,
}

async fn service_state(name: &str) -> String {
    match Command::new("systemctl")
        .arg("is-active")
        .arg(name)
        .output()
        .await
    {
        Ok(out) if out.status.success() => String::from_utf8_lossy(&out.stdout).trim().to_string(),
        Ok(out) => String::from_utf8_lossy(&out.stdout).trim().to_string(),
        Err(_) => "unavailable".to_string(),
    }
}

async fn load(path: &Path) -> Result<AgentConfig> {
    AgentConfig::load(path)
        .await
        .with_context(|| format!("read {}", path.display()))
}

async fn run_embedded_script(name: &str, script: &str, env: Option<(&str, &str)>) -> Result<()> {
    let mut command = Command::new("bash");
    command
        .arg("-s")
        .stdin(Stdio::piped())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());
    if let Some((key, value)) = env {
        command.env(key, value);
    }

    let mut child = command.spawn().with_context(|| format!("start {name}"))?;
    let mut stdin = child
        .stdin
        .take()
        .with_context(|| format!("open {name} stdin"))?;
    stdin
        .write_all(script.as_bytes())
        .await
        .with_context(|| format!("write {name}"))?;
    drop(stdin);

    let status = child
        .wait()
        .await
        .with_context(|| format!("wait for {name}"))?;
    if !status.success() {
        anyhow::bail!("{name} failed");
    }
    Ok(())
}

async fn run_first_server_smoke() -> Result<()> {
    run_embedded_script("first-server smoke test", FIRST_SERVER_SMOKE, None).await
}

async fn run_update_recovery() -> Result<()> {
    run_embedded_script(
        "interrupted Argus update recovery",
        INTERRUPTED_UPDATE_RECOVERY,
        None,
    )
    .await
}

async fn run_first_server_update(version: &str) -> Result<()> {
    run_update_recovery().await?;
    run_embedded_script(
        "transactional Argus update",
        FIRST_SERVER_UPDATE,
        Some(("ARGUS_TARGET_VERSION", version)),
    )
    .await
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Status => {
            let config = load(&cli.config).await?;
            println!(
                "Agent service: {}",
                service_state("argus-agent.service").await
            );
            println!(
                "Helper service: {}",
                service_state("argus-helper.service").await
            );
            println!("Agent ID: {}", config.agent_id);
            println!("Server ID: {}", config.server_id);
            println!("Control Plane: {}", config.control_plane_url);
            println!("Helper socket: {}", config.helper_socket.display());
        }
        Commands::Health => {
            let config = load(&cli.config).await?;
            let agent = service_state("argus-agent.service").await;
            let helper = service_state("argus-helper.service").await;
            let socket = UnixStream::connect(&config.helper_socket).await.is_ok();
            let snapshot =
                system::collect_snapshot(config.server_id, env!("CARGO_PKG_VERSION").to_string());
            println!("Agent: {agent}");
            println!("Helper: {helper}");
            println!("Helper socket: {}", if socket { "ok" } else { "failed" });
            println!(
                "System collection: {} / {} / {}",
                snapshot.hostname, snapshot.os, snapshot.kernel
            );
            if agent != "active" || helper != "active" || !socket {
                anyhow::bail!("local Argus health check failed");
            }
        }
        Commands::Connection => {
            let config = load(&cli.config).await?;
            let credential = config
                .credential
                .as_deref()
                .context("agent credential missing")?;
            let client = reqwest::Client::builder()
                .timeout(Duration::from_secs(10))
                .build()?;
            let response = client
                .get(format!("{}/agent/identity", config.control_plane_url))
                .bearer_auth(credential)
                .send()
                .await?;
            if !response.status().is_success() {
                anyhow::bail!(
                    "authenticated control-plane check failed: {}",
                    response.status()
                );
            }
            println!("control connection: authenticated");
        }
        Commands::Smoke => run_first_server_smoke().await?,
        Commands::Update { version } => run_first_server_update(&version).await?,
        Commands::RecoverUpdate => run_update_recovery().await?,
        Commands::System {
            command: SystemCommands::Info,
        } => {
            let snapshot =
                system::collect_snapshot(Uuid::nil(), env!("CARGO_PKG_VERSION").to_string());
            println!(
                "{}",
                serde_json::to_string_pretty(&json!({
                    "hostname": snapshot.hostname,
                    "os": snapshot.os,
                    "kernel": snapshot.kernel,
                    "architecture": snapshot.architecture,
                    "cpu_percent": snapshot.cpu_percent,
                    "ram_percent": snapshot.ram_percent,
                    "disk_percent": snapshot.disk_percent,
                    "load": snapshot.load,
                    "uptime_seconds": snapshot.uptime_seconds,
                }))?
            );
        }
        Commands::Version => println!("argusctl {}", env!("CARGO_PKG_VERSION")),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn update_defaults_to_main_discovery_tag() {
        let cli = Cli::try_parse_from(["argusctl", "update"]).expect("parse update command");
        match cli.command {
            Commands::Update { version } => assert_eq!(version, "main"),
            _ => panic!("expected update command"),
        }
    }

    #[test]
    fn update_accepts_explicit_immutable_revision() {
        let revision = "0123456789abcdef0123456789abcdef01234567";
        let cli = Cli::try_parse_from(["argusctl", "update", "--version", revision])
            .expect("parse pinned update command");
        match cli.command {
            Commands::Update { version } => assert_eq!(version, revision),
            _ => panic!("expected update command"),
        }
    }

    #[test]
    fn hidden_recovery_command_is_parseable_for_systemd() {
        let cli =
            Cli::try_parse_from(["argusctl", "recover-update"]).expect("parse recovery command");
        assert!(matches!(cli.command, Commands::RecoverUpdate));
    }
}
