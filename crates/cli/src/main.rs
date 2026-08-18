use anyhow::Result;
use clap::{Parser, Subcommand};
use serde_json::json;
use uuid::Uuid;

#[derive(Debug, Parser)]
#[command(name = "argusctl")]
#[command(about = "Argus local diagnostics CLI")]
struct Cli {
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

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Status => {
            println!("local agent status: unknown");
        }
        Commands::Health => {
            println!("local health: ok");
        }
        Commands::Connection => {
            println!("control connection: disconnected");
        }
        Commands::System {
            command: SystemCommands::Info,
        } => {
            let snapshot = system::collect_snapshot(Uuid::nil(), env!("CARGO_PKG_VERSION").to_string());
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
        Commands::Version => {
            println!("argusctl {}", env!("CARGO_PKG_VERSION"));
        }
    }

    Ok(())
}
