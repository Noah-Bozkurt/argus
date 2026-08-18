use agent::AgentConfig;
use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use serde_json::json;
use std::{
    path::{Path, PathBuf},
    time::Duration,
};
use tokio::{net::UnixStream, process::Command};
use uuid::Uuid;

#[derive(Debug, Parser)]
#[command(name = "argusctl", about = "Argus local diagnostics CLI")]
struct Cli {
    #[arg(
        long,
        env = "ARGUS_AGENT_CONFIG",
        default_value = "/etc/argus/agent.json"
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
