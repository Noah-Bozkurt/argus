use anyhow::{Context, Result, bail};
use clap::ValueEnum;
use cli::lifecycle;
use std::{path::Path, process::Stdio};
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    process::Command,
};

const CONTROL_PLANE_SERVICES: &[&str] = &[
    "web",
    "control-api",
    "worker",
    "content",
    "caddy",
    "postgres",
];
const HOST_UNITS: &[&str] = &["argus-agent.service", "argus-helper.service"];

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub(crate) enum LogTarget {
    /// Automatically show all runtime logs relevant to this host.
    All,
    /// Native Argus Agent and Helper journals.
    Host,
    /// Native Argus Agent journal.
    Agent,
    /// Native privileged Helper journal.
    Helper,
    /// All Docker Compose control-plane services.
    ControlPlane,
    /// Argus Web container.
    Web,
    /// Control API container.
    ControlApi,
    /// Background Worker container.
    Worker,
    /// Payload Content container.
    Content,
    /// Caddy reverse-proxy container.
    Caddy,
    /// PostgreSQL container.
    Postgres,
    /// Native installer log.
    Installer,
    /// Browser/host transactional update log.
    Update,
}

fn install_dir() -> std::path::PathBuf {
    lifecycle::env_path("ARGUS_INSTALL_DIR", lifecycle::DEFAULT_INSTALL_DIR)
}

fn log_dir() -> std::path::PathBuf {
    lifecycle::env_path("ARGUS_LOG_DIR", lifecycle::DEFAULT_LOG_DIR)
}

fn control_plane_installed() -> bool {
    let install_dir = install_dir();
    install_dir.join(".env").is_file() && install_dir.join("compose.yaml").is_file()
}

fn redact_line(line: &str) -> &str {
    let lower = line.to_ascii_lowercase();
    if [
        "authorization:",
        "password=",
        "token=",
        "secret=",
        "database_url=",
        "bearer ",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
    {
        "[redacted sensitive log line]"
    } else {
        line
    }
}

async fn stream_command(program: &str, args: &[String], description: &str) -> Result<()> {
    let mut child = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| format!("start {description}"))?;
    let stdout = child
        .stdout
        .take()
        .with_context(|| format!("capture {description} stdout"))?;
    let stderr = child
        .stderr
        .take()
        .with_context(|| format!("capture {description} stderr"))?;
    let mut stdout = BufReader::new(stdout).lines();
    let mut stderr = BufReader::new(stderr).lines();
    let mut stdout_open = true;
    let mut stderr_open = true;

    while stdout_open || stderr_open {
        tokio::select! {
            line = stdout.next_line(), if stdout_open => {
                match line.with_context(|| format!("read {description} stdout"))? {
                    Some(line) => println!("{}", redact_line(&line)),
                    None => stdout_open = false,
                }
            }
            line = stderr.next_line(), if stderr_open => {
                match line.with_context(|| format!("read {description} stderr"))? {
                    Some(line) => eprintln!("{}", redact_line(&line)),
                    None => stderr_open = false,
                }
            }
        }
    }

    let status = child
        .wait()
        .await
        .with_context(|| format!("wait for {description}"))?;
    if !status.success() {
        bail!("{description} failed with {status}");
    }
    Ok(())
}

async fn journal_logs(units: &[&str], tail: u32, follow: bool, since: Option<&str>) -> Result<()> {
    let mut args = vec![
        "--no-pager".to_string(),
        "--output=short-iso".to_string(),
        "--lines".to_string(),
        tail.to_string(),
    ];
    if follow {
        args.push("--follow".into());
    }
    if let Some(value) = since {
        args.push("--since".into());
        args.push(value.to_string());
    }
    for unit in units {
        args.push("--unit".into());
        args.push((*unit).to_string());
    }
    stream_command("journalctl", &args, "Argus journal logs").await
}

fn compose_services(target: LogTarget) -> Option<&'static [&'static str]> {
    match target {
        LogTarget::ControlPlane => Some(CONTROL_PLANE_SERVICES),
        LogTarget::Web => Some(&["web"]),
        LogTarget::ControlApi => Some(&["control-api"]),
        LogTarget::Worker => Some(&["worker"]),
        LogTarget::Content => Some(&["content"]),
        LogTarget::Caddy => Some(&["caddy"]),
        LogTarget::Postgres => Some(&["postgres"]),
        _ => None,
    }
}

async fn compose_logs(
    target: LogTarget,
    tail: u32,
    follow: bool,
    since: Option<&str>,
) -> Result<()> {
    if !control_plane_installed() {
        bail!("this host does not have an installed Argus control plane");
    }
    let services = compose_services(target).context("invalid control-plane log target")?;
    let install_dir = install_dir();
    let mut args = vec![
        "compose".to_string(),
        "--project-directory".to_string(),
        install_dir.display().to_string(),
        "--env-file".to_string(),
        install_dir.join(".env").display().to_string(),
        "-f".to_string(),
        install_dir.join("compose.yaml").display().to_string(),
        "logs".to_string(),
        "--no-color".to_string(),
        "--tail".to_string(),
        tail.to_string(),
    ];
    if follow {
        args.push("--follow".into());
    }
    if let Some(value) = since {
        args.push("--since".into());
        args.push(value.to_string());
    }
    args.extend(services.iter().map(|service| (*service).to_string()));
    stream_command("docker", &args, "Argus control-plane logs").await
}

async fn file_logs(path: &Path, tail: u32, follow: bool, since: Option<&str>) -> Result<()> {
    if since.is_some() {
        bail!("--since is not supported for installer/update file logs");
    }
    if !path.is_file() {
        bail!("log file does not exist: {}", path.display());
    }
    let mut args = vec!["-n".to_string(), tail.to_string()];
    if follow {
        args.push("-f".into());
    }
    args.push(path.display().to_string());
    stream_command("tail", &args, &format!("log file {}", path.display())).await
}

async fn all_logs(tail: u32, follow: bool, since: Option<&str>) -> Result<()> {
    if control_plane_installed() {
        if follow {
            println!("Following native and control-plane Argus logs (Ctrl-C to stop)…");
            let (host, control_plane) = tokio::join!(
                journal_logs(HOST_UNITS, tail, true, since),
                compose_logs(LogTarget::ControlPlane, tail, true, since)
            );
            host?;
            control_plane?;
            Ok(())
        } else {
            println!("== Native services ==");
            journal_logs(HOST_UNITS, tail, false, since).await?;
            println!("\n== Control plane ==");
            compose_logs(LogTarget::ControlPlane, tail, false, since).await
        }
    } else {
        journal_logs(HOST_UNITS, tail, follow, since).await
    }
}

pub(crate) async fn run(
    target: Option<LogTarget>,
    tail: u32,
    follow: bool,
    since: Option<&str>,
) -> Result<()> {
    if tail == 0 || tail > 100_000 {
        bail!("--tail must be between 1 and 100000 lines");
    }
    match target.unwrap_or(LogTarget::All) {
        LogTarget::All => all_logs(tail, follow, since).await,
        LogTarget::Host => journal_logs(HOST_UNITS, tail, follow, since).await,
        LogTarget::Agent => journal_logs(&["argus-agent.service"], tail, follow, since).await,
        LogTarget::Helper => journal_logs(&["argus-helper.service"], tail, follow, since).await,
        target @ (LogTarget::ControlPlane
        | LogTarget::Web
        | LogTarget::ControlApi
        | LogTarget::Worker
        | LogTarget::Content
        | LogTarget::Caddy
        | LogTarget::Postgres) => compose_logs(target, tail, follow, since).await,
        LogTarget::Installer => {
            file_logs(&log_dir().join("installer.log"), tail, follow, since).await
        }
        LogTarget::Update => file_logs(&log_dir().join("update.log"), tail, follow, since).await,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_common_secret_shapes() {
        for line in [
            "Authorization: Bearer abc",
            "password=hunter2",
            "token=abc",
            "SECRET=value",
            "DATABASE_URL=postgresql://secret",
            "request bearer abc",
        ] {
            assert_eq!(redact_line(line), "[redacted sensitive log line]");
        }
        assert_eq!(redact_line("control-api started"), "control-api started");
    }

    #[test]
    fn compose_targets_map_to_expected_service_names() {
        assert_eq!(compose_services(LogTarget::Web), Some(&["web"][..]));
        assert_eq!(
            compose_services(LogTarget::ControlApi),
            Some(&["control-api"][..])
        );
        assert_eq!(
            compose_services(LogTarget::ControlPlane),
            Some(CONTROL_PLANE_SERVICES)
        );
        assert_eq!(compose_services(LogTarget::Host), None);
    }
}
