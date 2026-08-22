use agent::AgentConfig;
use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use cli::{domain, lifecycle};
use serde_json::json;
use std::{
    io::{self, IsTerminal},
    path::{Path, PathBuf},
    process::Stdio,
    time::Duration,
};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::UnixStream,
    process::Command,
};
use uuid::Uuid;

mod doctor;
mod logs;

const FIRST_SERVER_SMOKE: &str = include_str!("../../../scripts/first-server-smoke.sh");
const FIRST_SERVER_UPDATE: &str = include_str!("../../../scripts/update-first-test.sh");
const INTERRUPTED_UPDATE_RECOVERY: &str =
    include_str!("../../../scripts/recover-interrupted-update.sh");
const UNINSTALL: &str = include_str!("../../../scripts/uninstall.sh");
const REGISTRY_LOGIN: &str = include_str!("../../../scripts/registry-login.sh");

#[derive(Debug, Parser)]
#[command(name = "argusctl", about = "Argus local diagnostics and lifecycle CLI")]
struct Cli {
    /// Print machine-readable JSON where supported.
    #[arg(long)]
    json: bool,
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
    /// Run comprehensive, read-only installation diagnostics.
    Doctor {
        /// Skip DNS and public HTTPS checks.
        #[arg(long)]
        offline: bool,
        /// Include additional explanatory output.
        #[arg(long, short = 'v')]
        verbose: bool,
    },
    /// Restore missing or damaged Argus files without changing versions or data.
    Repair {
        /// Skip the interactive repair confirmation.
        #[arg(long)]
        yes: bool,
    },
    /// Show the stored initial operator login (requires root).
    Credentials,
    /// Show local service and enrollment state.
    Status,
    /// View secret-safe Argus runtime and lifecycle logs.
    Logs {
        /// Log source. Omit to automatically show the relevant runtime logs for this host.
        #[arg(value_enum)]
        target: Option<logs::LogTarget>,
        /// Number of recent lines to show per source.
        #[arg(long, default_value_t = 200)]
        tail: u32,
        /// Continue following new log output.
        #[arg(long, short = 'f')]
        follow: bool,
        /// Only show entries since a journald/Compose time such as 1h or 2026-08-22T08:00:00.
        #[arg(long)]
        since: Option<String>,
    },
    #[command(hide = true)]
    /// Check native services, the helper socket, and host collection.
    Health,
    #[command(hide = true)]
    /// Verify the Agent can authenticate with its control plane.
    Connection,
    #[command(hide = true)]
    /// Run the full installed control-plane smoke test.
    Smoke,
    /// Transactionally update Argus to a release tag or immutable revision.
    Update {
        /// Release tag or full 40-character Git revision.
        #[arg(long, default_value = "main")]
        version: String,
        /// Show complete, secret-safe update diagnostics.
        #[arg(long, short = 'v')]
        verbose: bool,
        /// Skip the interactive update confirmation.
        #[arg(long)]
        yes: bool,
    },
    /// Remove Argus, preserving persistent data unless purge is requested.
    Uninstall {
        /// Skip interactive confirmation (does not imply data purge).
        #[arg(long)]
        yes: bool,
        /// Permanently delete data, backups, logs, and Docker volumes.
        #[arg(long)]
        purge_data: bool,
    },
    /// Validate and store replacement GHCR credentials.
    RegistryLogin {
        /// GitHub username; the token is still entered securely.
        #[arg(long)]
        username: Option<String>,
    },
    /// Inspect or change the public web and content domains.
    Domain {
        #[command(subcommand)]
        command: DomainCommands,
    },
    #[command(hide = true)]
    RecoverUpdate {
        #[arg(long)]
        retry_failed: bool,
    },
    /// Display local host information.
    System {
        #[command(subcommand)]
        command: SystemCommands,
    },
    /// Print the argusctl build version.
    Version,
}

#[derive(Debug, Subcommand)]
enum DomainCommands {
    /// Show the configured web and content domains.
    Show,
    /// Resolve both configured domains through DNS.
    Check,
    /// Transactionally change both public domains.
    Set {
        /// New primary web domain.
        domain: String,
        /// New content domain (defaults to content.<DOMAIN>).
        #[arg(long)]
        content_domain: Option<String>,
        /// Skip the interactive confirmation.
        #[arg(long)]
        yes: bool,
    },
}

#[derive(Debug, Subcommand)]
enum SystemCommands {
    /// Print a JSON host-resource snapshot.
    Info,
}

struct UpdateUi {
    color: bool,
    post_start: bool,
    download_announced: bool,
    rollback_started: bool,
    rollback_completed: bool,
    finished: bool,
}

impl UpdateUi {
    fn new() -> Self {
        Self {
            color: io::stdout().is_terminal() && std::env::var_os("NO_COLOR").is_none(),
            post_start: false,
            download_announced: false,
            rollback_started: false,
            rollback_completed: false,
            finished: false,
        }
    }

    fn paint(&self, code: &str, text: &str) -> String {
        if self.color {
            format!("\x1b[{code}m{text}\x1b[0m")
        } else {
            text.to_string()
        }
    }

    fn begin(&self, requested: &str) {
        println!("{}", self.paint("1;36", "Argus update"));
        self.detail(&format!("Requested version: {requested}"));
        println!();
    }

    fn step(&self, message: &str) {
        println!("{} {message}", self.paint("36", "  ›"));
    }

    fn success(&self, message: &str) {
        println!("{} {message}", self.paint("32", "  ✓"));
    }

    fn warning(&self, message: &str) {
        eprintln!("{} {message}", self.paint("33", "  !"));
    }

    fn error(&self, message: &str) {
        eprintln!("{} {message}", self.paint("31", "  ✗"));
    }

    fn detail(&self, message: &str) {
        println!("{}", self.paint("2", &format!("    {message}")));
    }

    fn short_revision(revision: &str) -> &str {
        revision.get(..12).unwrap_or(revision)
    }

    fn handle_line(&mut self, line: &str) {
        let line = line.trim();
        if line.is_empty() {
            return;
        }

        if let Some(revision) = line.strip_prefix("[argus-update] current installed revision: ") {
            self.step(&format!(
                "Checking current installation ({})",
                Self::short_revision(revision)
            ));
            return;
        }
        if line == "[argus-update] verifying current installation before update" {
            return;
        }
        if line.starts_with("Argus first-server smoke test passed:") {
            if self.post_start {
                self.success("Updated installation is healthy");
            } else {
                self.success("Current installation is healthy");
            }
            return;
        }
        if line.starts_with("[argus-update] resolving target '") {
            self.step("Checking for updates");
            return;
        }
        if line.starts_with("[argus-update] pre-fetching ") {
            if !self.download_announced {
                self.download_announced = true;
                self.step("Downloading update");
            }
            return;
        }
        if let Some(revision) = line.strip_prefix("[argus-update] resolved target revision: ") {
            self.success(&format!(
                "Update downloaded ({})",
                Self::short_revision(revision)
            ));
            return;
        }
        if let Some(revision) =
            line.strip_prefix("[argus-update] already running requested revision ")
        {
            self.success(&format!(
                "Already up to date ({})",
                Self::short_revision(revision)
            ));
            self.finished = true;
            return;
        }
        if line.starts_with("[argus-update] storage preflight: ") {
            self.step("Checking backup storage");
            return;
        }
        if line == "[argus-update] quiescing native Agent/Helper and control-plane writers" {
            self.success("Backup storage is sufficient");
            self.step("Stopping Argus services");
            return;
        }
        if line == "[argus-update] creating consistent PostgreSQL backup" {
            self.step("Creating rollback backup");
            return;
        }
        if line == "[argus-update] installing target deployment assets and native binaries" {
            self.step("Installing update");
            return;
        }
        if let Some(revision) = line.strip_prefix("[argus-update] starting target control plane ") {
            self.post_start = true;
            self.step(&format!(
                "Starting Argus {}",
                Self::short_revision(revision)
            ));
            return;
        }
        if self.post_start && line == "[argus-smoke] validating deployed configuration" {
            self.step("Verifying updated installation");
            return;
        }
        if let Some(change) = line.strip_prefix("[argus-update] update succeeded: ") {
            let change = change
                .split(" -> ")
                .map(Self::short_revision)
                .collect::<Vec<_>>()
                .join(" → ");
            println!();
            self.success(&format!("Update complete: {change}"));
            self.finished = true;
            return;
        }
        if let Some(path) = line.strip_prefix("[argus-update] rollback snapshot retained at ") {
            self.detail(&format!("Rollback snapshot: {path}"));
            return;
        }
        if let Some(message) = line.strip_prefix("[argus-update] warning: ") {
            if message.starts_with("update failed; automatically rolling back transaction ") {
                self.rollback_started = true;
                println!();
                self.warning("Update failed; restoring the previous version");
            } else if message.starts_with("rollback completed successfully; restored revision ") {
                self.rollback_completed = true;
                let revision = message
                    .trim_start_matches("rollback completed successfully; restored revision ");
                self.success(&format!(
                    "Rollback completed ({})",
                    Self::short_revision(revision)
                ));
            } else {
                self.warning(message);
            }
            return;
        }
        if let Some(message) = line.strip_prefix("[argus-update] error: ") {
            self.error(message);
            return;
        }
        if let Some(message) = line.strip_prefix("[argus-smoke] FAIL: ") {
            self.error(message);
            return;
        }

        let lower = line.to_ascii_lowercase();
        if lower.starts_with("error:")
            || lower.starts_with("error response from daemon")
            || lower.contains("permission denied")
        {
            self.error(line);
        }
    }

    fn finish_failure(&self) {
        if !self.finished {
            println!();
            if self.rollback_completed {
                self.error("Update did not complete; the previous version was restored");
            } else if self.rollback_started {
                self.error("Update and automatic rollback did not complete cleanly");
            } else {
                self.error("Update failed");
            }
            self.detail("Re-run with --verbose for full diagnostics");
        }
    }
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

async fn run_embedded_script(name: &str, script: &str, env: &[(&str, &str)]) -> Result<()> {
    let mut command = Command::new("bash");
    command
        .arg("-s")
        .stdin(Stdio::piped())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());
    for (key, value) in env {
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

async fn run_concise_update_script(version: &str) -> Result<()> {
    let name = "transactional Argus update";
    let mut command = Command::new("bash");
    command
        .arg("-s")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env("ARGUS_TARGET_VERSION", version);

    let mut child = command.spawn().with_context(|| format!("start {name}"))?;
    let mut stdin = child
        .stdin
        .take()
        .with_context(|| format!("open {name} stdin"))?;
    stdin
        .write_all(FIRST_SERVER_UPDATE.as_bytes())
        .await
        .with_context(|| format!("write {name}"))?;
    drop(stdin);

    let stdout = child
        .stdout
        .take()
        .with_context(|| format!("capture {name} stdout"))?;
    let stderr = child
        .stderr
        .take()
        .with_context(|| format!("capture {name} stderr"))?;
    let mut stdout = BufReader::new(stdout).lines();
    let mut stderr = BufReader::new(stderr).lines();
    let mut stdout_open = true;
    let mut stderr_open = true;
    let mut ui = UpdateUi::new();
    ui.begin(version);

    while stdout_open || stderr_open {
        tokio::select! {
            line = stdout.next_line(), if stdout_open => {
                match line.with_context(|| format!("read {name} stdout"))? {
                    Some(line) => ui.handle_line(&line),
                    None => stdout_open = false,
                }
            }
            line = stderr.next_line(), if stderr_open => {
                match line.with_context(|| format!("read {name} stderr"))? {
                    Some(line) => ui.handle_line(&line),
                    None => stderr_open = false,
                }
            }
        }
    }

    let status = child
        .wait()
        .await
        .with_context(|| format!("wait for {name}"))?;
    if !status.success() {
        ui.finish_failure();
        anyhow::bail!("{name} failed");
    }
    Ok(())
}

async fn run_first_server_smoke() -> Result<()> {
    run_embedded_script("first-server smoke test", FIRST_SERVER_SMOKE, &[]).await
}

async fn run_update_recovery(retry_failed: bool) -> Result<()> {
    let env = retry_failed.then_some(("ARGUS_UPDATE_RECOVERY_RETRY_FAILED", "1"));
    run_embedded_script(
        "interrupted Argus update recovery",
        INTERRUPTED_UPDATE_RECOVERY,
        env.as_slice(),
    )
    .await
}

async fn run_first_server_update(version: &str, verbose: bool) -> Result<()> {
    if std::env::var_os("ARGUS_UPDATE_DELEGATED_REVISION").is_none() {
        run_update_recovery(false).await?;
    }
    if verbose {
        run_embedded_script(
            "transactional Argus update",
            FIRST_SERVER_UPDATE,
            &[("ARGUS_TARGET_VERSION", version)],
        )
        .await
    } else {
        run_concise_update_script(version).await
    }
}

fn confirm_action(message: &str, yes: bool) -> Result<()> {
    if yes {
        return Ok(());
    }
    if !lifecycle::interactive_available() {
        anyhow::bail!("confirmation required; rerun with --yes");
    }
    println!("{message}");
    let answer = lifecycle::prompt_line("Continue? [y/N]: ")?;
    if !matches!(answer.to_ascii_lowercase().as_str(), "y" | "yes") {
        anyhow::bail!("operation cancelled");
    }
    Ok(())
}

async fn run_uninstall(yes: bool, purge_data: bool) -> Result<()> {
    let mut env = Vec::new();
    if yes {
        env.push(("ARGUS_UNINSTALL_CONFIRM", "1"));
    }
    if purge_data {
        env.push(("ARGUS_UNINSTALL_PURGE_DATA", "1"));
    }
    run_embedded_script("Argus uninstall", UNINSTALL, &env).await
}

async fn run_registry_login(username: Option<&str>) -> Result<()> {
    let mut env = Vec::new();
    if let Some(value) = username {
        env.push(("ARGUS_REGISTRY_USERNAME_OVERRIDE", value));
    }
    run_embedded_script("Argus registry login", REGISTRY_LOGIN, &env).await
}

async fn run_repair() -> Result<()> {
    let status = Command::new("/usr/local/bin/argus-installer")
        .args(["--mode", "repair"])
        .status()
        .await
        .context("start Argus repair; if the installer is missing, run the public installer")?;
    if !status.success() {
        anyhow::bail!(
            "Argus repair failed; run the public installer if the local installer is damaged"
        );
    }
    Ok(())
}

fn show_credentials() -> Result<()> {
    lifecycle::require_root().context("stored credentials may only be shown by root")?;
    let install_dir = lifecycle::env_path("ARGUS_INSTALL_DIR", lifecycle::DEFAULT_INSTALL_DIR);
    let values = lifecycle::read_env_file(&install_dir.join(".env"))?;
    let username = values
        .get("ARGUS_BASIC_AUTH_USER")
        .context("stored login username is missing")?;
    let password = values
        .get("ARGUS_BASIC_AUTH_PASSWORD")
        .context("stored login password is missing")?;
    println!("Login username: {username}");
    println!("Login password: {password}");
    println!("Keep this output private.");
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Doctor { offline, verbose } => {
            let report = doctor::run(offline).await;
            if cli.json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                report.print_human(verbose);
            }
            if !report.healthy {
                if cli.json {
                    std::process::exit(1);
                }
                anyhow::bail!("Argus doctor found problems");
            }
        }
        Commands::Repair { yes } => {
            confirm_action(
                "Argus will restore its installed files and services without changing data or versions.",
                yes,
            )?;
            run_repair().await?;
        }
        Commands::Credentials => show_credentials()?,
        Commands::Status => {
            let config = load(&cli.config).await?;
            let agent = service_state("argus-agent.service").await;
            let helper = service_state("argus-helper.service").await;
            if cli.json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&json!({
                        "agent_service": agent,
                        "helper_service": helper,
                        "agent_id": config.agent_id,
                        "server_id": config.server_id,
                        "control_plane": config.control_plane_url,
                        "helper_socket": config.helper_socket,
                    }))?
                );
            } else {
                println!("Agent service: {agent}");
                println!("Helper service: {helper}");
                println!("Agent ID: {}", config.agent_id);
                println!("Server ID: {}", config.server_id);
                println!("Control Plane: {}", config.control_plane_url);
                println!("Helper socket: {}", config.helper_socket.display());
            }
        }
        Commands::Logs {
            target,
            tail,
            follow,
            since,
        } => {
            if cli.json {
                anyhow::bail!("--json is not supported for streaming logs");
            }
            logs::run(target, tail, follow, since.as_deref()).await?;
        }
        Commands::Health => {
            let config = load(&cli.config).await?;
            let agent = service_state("argus-agent.service").await;
            let helper = service_state("argus-helper.service").await;
            let socket = UnixStream::connect(&config.helper_socket).await.is_ok();
            let snapshot =
                system::collect_snapshot(config.server_id, env!("CARGO_PKG_VERSION").to_string());
            if cli.json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&json!({
                        "healthy": agent == "active" && helper == "active" && socket,
                        "agent": agent,
                        "helper": helper,
                        "helper_socket": socket,
                        "system": {
                            "hostname": snapshot.hostname,
                            "os": snapshot.os,
                            "kernel": snapshot.kernel,
                        }
                    }))?
                );
            } else {
                println!("Agent: {agent}");
                println!("Helper: {helper}");
                println!("Helper socket: {}", if socket { "ok" } else { "failed" });
                println!(
                    "System collection: {} / {} / {}",
                    snapshot.hostname, snapshot.os, snapshot.kernel
                );
            }
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
            if cli.json {
                println!("{}", json!({"authenticated": true}));
            } else {
                println!("control connection: authenticated");
            }
        }
        Commands::Smoke => run_first_server_smoke().await?,
        Commands::Update {
            version,
            verbose,
            yes,
        } => {
            confirm_action(
                &format!(
                    "Argus will create a rollback snapshot and update this host to '{version}'."
                ),
                yes,
            )?;
            run_first_server_update(&version, verbose).await?
        }
        Commands::Uninstall { yes, purge_data } => run_uninstall(yes, purge_data).await?,
        Commands::RegistryLogin { username } => run_registry_login(username.as_deref()).await?,
        Commands::Domain { command } => match command {
            DomainCommands::Show => {
                let domains = domain::installed_domains()?;
                if cli.json {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(
                            &json!({"web": domains.web, "content": domains.content})
                        )?
                    );
                } else {
                    println!("Web domain: {}", domains.web);
                    println!("Content domain: {}", domains.content);
                }
            }
            DomainCommands::Check => {
                let resolutions = domain::check_installed_domains()?;
                if cli.json {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(
                            &resolutions
                                .iter()
                                .map(|resolution| json!({
                                    "domain": resolution.domain,
                                    "addresses": resolution.addresses,
                                }))
                                .collect::<Vec<_>>()
                        )?
                    );
                } else {
                    for resolution in resolutions {
                        let addresses = resolution
                            .addresses
                            .iter()
                            .map(ToString::to_string)
                            .collect::<Vec<_>>()
                            .join(", ");
                        println!("{} -> {addresses}", resolution.domain);
                    }
                }
            }
            DomainCommands::Set {
                domain: web_domain,
                content_domain,
                yes,
            } => {
                let old = domain::installed_domains()?;
                let new = domain::normalize_domains(&web_domain, content_domain.as_deref())?;
                confirm_action(
                    &format!(
                        "Change Argus domains?\n  Web:     {} → {}\n  Content: {} → {}",
                        old.web, new.web, old.content, new.content
                    ),
                    yes,
                )?;
                let domains =
                    domain::set_installed_domains(&web_domain, content_domain.as_deref())?;
                println!("Domain change complete.");
                println!("Web domain: {}", domains.web);
                println!("Content domain: {}", domains.content);
            }
        },
        Commands::RecoverUpdate { retry_failed } => run_update_recovery(retry_failed).await?,
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
        Commands::Version => {
            if cli.json {
                println!("{}", json!({"version": env!("CARGO_PKG_VERSION")}));
            } else {
                println!("argusctl {}", env!("CARGO_PKG_VERSION"));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn update_defaults_to_main_discovery_tag() {
        let cli = Cli::try_parse_from(["argusctl", "update"]).expect("parse update command");
        match cli.command {
            Commands::Update {
                version, verbose, ..
            } => {
                assert_eq!(version, "main");
                assert!(!verbose);
            }
            _ => panic!("expected update command"),
        }
    }

    #[test]
    fn update_accepts_explicit_immutable_revision() {
        let revision = "0123456789abcdef0123456789abcdef01234567";
        let cli = Cli::try_parse_from(["argusctl", "update", "--version", revision])
            .expect("parse pinned update command");
        match cli.command {
            Commands::Update {
                version, verbose, ..
            } => {
                assert_eq!(version, revision);
                assert!(!verbose);
            }
            _ => panic!("expected update command"),
        }
    }

    #[test]
    fn update_verbose_flag_is_parseable() {
        let cli = Cli::try_parse_from(["argusctl", "update", "--verbose"])
            .expect("parse verbose update command");
        assert!(matches!(
            cli.command,
            Commands::Update { verbose: true, .. }
        ));
    }

    #[test]
    fn logs_accept_target_follow_tail_and_since() {
        let cli = Cli::try_parse_from([
            "argusctl",
            "logs",
            "control-plane",
            "--tail",
            "500",
            "--follow",
            "--since",
            "1h",
        ])
        .expect("parse logs command");
        assert!(matches!(
            cli.command,
            Commands::Logs {
                target: Some(logs::LogTarget::ControlPlane),
                tail: 500,
                follow: true,
                since: Some(value),
            } if value == "1h"
        ));
    }

    #[test]
    fn compatibility_diagnostics_remain_parseable_but_hidden_from_help() {
        for name in ["health", "connection", "smoke"] {
            Cli::try_parse_from(["argusctl", name]).expect("parse compatibility command");
        }
        let mut command = Cli::command();
        let mut buffer = Vec::new();
        command
            .write_long_help(&mut buffer)
            .expect("render argusctl help");
        let help = String::from_utf8(buffer).expect("help is UTF-8");
        assert!(help.lines().any(|line| line.trim_start().starts_with("logs ")));
        for hidden in ["health", "connection", "smoke"] {
            assert!(
                !help
                    .lines()
                    .any(|line| line.trim_start().starts_with(&format!("{hidden} "))),
                "{hidden} should not be promoted in top-level help"
            );
        }
    }

    #[test]
    fn hidden_recovery_command_is_parseable_for_systemd() {
        let cli =
            Cli::try_parse_from(["argusctl", "recover-update"]).expect("parse recovery command");
        assert!(matches!(
            cli.command,
            Commands::RecoverUpdate {
                retry_failed: false
            }
        ));
    }

    #[test]
    fn failed_recovery_retry_requires_explicit_flag() {
        let cli = Cli::try_parse_from(["argusctl", "recover-update", "--retry-failed"])
            .expect("parse failed recovery retry");
        assert!(matches!(
            cli.command,
            Commands::RecoverUpdate { retry_failed: true }
        ));
    }

    #[test]
    fn uninstall_requires_explicit_flags_for_noninteractive_or_purge_behavior() {
        let cli = Cli::try_parse_from(["argusctl", "uninstall", "--yes", "--purge-data"])
            .expect("parse uninstall command");
        assert!(matches!(
            cli.command,
            Commands::Uninstall {
                yes: true,
                purge_data: true
            }
        ));
    }

    #[test]
    fn registry_login_accepts_a_username_without_a_token_argument() {
        let cli = Cli::try_parse_from(["argusctl", "registry-login", "--username", "octocat"])
            .expect("parse registry login command");
        assert!(
            matches!(cli.command, Commands::RegistryLogin { username: Some(value) } if value == "octocat")
        );
    }

    #[test]
    fn domain_set_accepts_optional_content_domain() {
        let cli = Cli::try_parse_from([
            "argusctl",
            "domain",
            "set",
            "argus.example.com",
            "--content-domain",
            "content.argus.example.com",
        ])
        .expect("parse domain set command");
        assert!(matches!(
            cli.command,
            Commands::Domain {
                command: DomainCommands::Set {
                    domain,
                    content_domain: Some(content_domain),
                    ..
                }
            } if domain == "argus.example.com" && content_domain == "content.argus.example.com"
        ));
    }

    #[test]
    fn update_ui_shortens_full_revisions() {
        assert_eq!(
            UpdateUi::short_revision("0123456789abcdef0123456789abcdef01234567"),
            "0123456789ab"
        );
        assert_eq!(UpdateUi::short_revision("main"), "main");
    }
}
