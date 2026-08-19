use helper::HelperError;
use std::{
    path::{Path, PathBuf},
    process::Stdio,
};
use tokio::{io::AsyncWriteExt, process::Command};

const ROLLBACK_SECONDS: &str = "120s";
const MANAGED_PATHS: [&str; 4] = [
    "etc/ssh/sshd_config",
    "etc/ssh/sshd_config.d",
    "etc/ufw",
    "etc/apt/apt.conf.d/20auto-upgrades",
];

pub async fn apply(restore_id: &str, backup: &str) -> Result<String, HelperError> {
    validate_restore_id(restore_id)?;
    helper::HelperApi::validate_backup_reference(backup)?;

    // Never trust a previous UI/API preflight. Re-run it immediately before
    // creating the rollback transaction and touching live configuration.
    super::restore_preflight::run(restore_id, backup).await?;

    let backup_dir = backup_dir();
    let archive = backup_dir.join(backup);
    let archive_text = archive.to_string_lossy().into_owned();
    let candidate_entries = run_capture("tar", &["-tzf", &archive_text]).await?;
    if !candidate_entries
        .lines()
        .any(|entry| entry == "etc/ufw" || entry.starts_with("etc/ufw/"))
    {
        return Err(HelperError::InvalidRequest);
    }

    let transaction = transaction_dir(restore_id);
    let _ = tokio::fs::remove_dir_all(&transaction).await;
    ensure_private_dir(&transaction).await?;
    create_rollback_archive(&transaction).await?;

    let firewall_was_active = ufw_is_active().await?;
    tokio::fs::write(
        transaction.join("firewall-state"),
        if firewall_was_active {
            "active\n"
        } else {
            "inactive\n"
        },
    )
    .await
    .map_err(system_error)?;

    schedule_rollback(restore_id).await?;

    let result = apply_candidate(&archive, firewall_was_active).await;
    match result {
        Ok(ports) => serde_json::to_string(&serde_json::json!({
            "backup": backup,
            "rollback_armed": true,
            "rollback_seconds": 120,
            "ssh_ports": ports,
            "firewall_runtime_preserved": true,
        }))
        .map_err(|error| HelperError::SystemCommandFailed(error.to_string())),
        Err(apply_error) => match rollback(restore_id).await {
            Ok(()) => Err(apply_error),
            Err(rollback_error) => Err(HelperError::SystemCommandFailed(format!(
                "restore failed ({apply_error}); immediate rollback also failed ({rollback_error}); timed rollback remains armed"
            ))),
        },
    }
}

pub async fn commit(restore_id: &str) -> Result<(), HelperError> {
    validate_restore_id(restore_id)?;
    let timer = format!("{}.timer", rollback_unit(restore_id));
    let status = Command::new("systemctl")
        .args(["is-active", "--quiet", &timer])
        .status()
        .await
        .map_err(|error| map_spawn("systemctl", error))?;
    if status.success() {
        run_command("systemctl", &["stop", &timer]).await?;
    }
    let transaction = transaction_dir(restore_id);
    let _ = tokio::fs::remove_dir_all(transaction).await;
    Ok(())
}

pub async fn rollback(restore_id: &str) -> Result<(), HelperError> {
    validate_restore_id(restore_id)?;
    let transaction = transaction_dir(restore_id);
    let rollback_archive = transaction.join("rollback.tar.gz");
    if tokio::fs::metadata(&rollback_archive).await.is_err() {
        return Err(HelperError::InvalidRequest);
    }

    // Do not disarm the timer before rollback has succeeded. If a synchronous
    // rollback fails halfway, the independent timer must retain its chance to
    // retry the recovery path.
    remove_managed_paths().await?;
    extract_rollback_archive(&rollback_archive).await?;

    let previous_firewall = tokio::fs::read_to_string(transaction.join("firewall-state"))
        .await
        .map_err(system_error)?;
    if previous_firewall.trim() == "active" {
        run_command("ufw", &["--force", "enable"]).await?;
        run_command("ufw", &["reload"]).await?;
    } else {
        run_command("ufw", &["--force", "disable"]).await?;
    }

    // UFW enable/disable may update ufw.conf. Re-extract the rollback archive
    // so persistent configuration is exactly what existed before the restore,
    // while keeping the recovered runtime firewall state.
    extract_rollback_archive(&rollback_archive).await?;
    run_command("sshd", &["-t"]).await?;
    reload_ssh().await?;

    let timer = format!("{}.timer", rollback_unit(restore_id));
    let _ = Command::new("systemctl").args(["stop", &timer]).status().await;
    let _ = tokio::fs::remove_dir_all(transaction).await;
    Ok(())
}

async fn apply_candidate(
    archive: &Path,
    firewall_was_active: bool,
) -> Result<Vec<u16>, HelperError> {
    remove_managed_paths().await?;
    let archive_text = archive.to_string_lossy().into_owned();
    run_command("tar", &["-xzf", &archive_text, "-C", "/"]).await?;

    // Validate the actual live files after extraction, not only staging.
    run_command("sshd", &["-t"]).await?;
    let apt_config = "/etc/apt/apt.conf.d/20auto-upgrades";
    run_command("apt-config", &["-c", apt_config, "dump"]).await?;
    validate_live_ufw_rules().await?;

    let sshd = run_capture("sshd", &["-T"]).await?;
    let ports = parse_sshd_ports(&sshd)?;

    if firewall_was_active {
        for port in &ports {
            let rule = format!("{port}/tcp");
            run_command(
                "ufw",
                &[
                    "allow",
                    &rule,
                    "comment",
                    "Argus restored SSH safety rule",
                ],
            )
            .await?;
        }
        // Preserve the pre-restore runtime firewall state even if the backup's
        // ufw.conf says otherwise. The SSH safety rules intentionally remain in
        // the restored policy so a future reload/reboot cannot close the new port.
        run_command("ufw", &["--force", "enable"]).await?;
        run_command("ufw", &["reload"]).await?;
    } else {
        run_command("ufw", &["--force", "disable"]).await?;
    }

    reload_ssh().await?;
    verify_listeners(&ports).await?;
    Ok(ports)
}

async fn create_rollback_archive(transaction: &Path) -> Result<(), HelperError> {
    let rollback = transaction.join("rollback.tar.gz");
    let rollback_text = rollback.to_string_lossy().into_owned();
    let mut args = vec![
        "-C".to_string(),
        "/".to_string(),
        "-czf".to_string(),
        rollback_text,
    ];
    for relative in MANAGED_PATHS {
        let live = Path::new("/").join(relative);
        if tokio::fs::symlink_metadata(&live).await.is_ok() {
            args.push(relative.to_string());
        }
    }
    if args.len() == 4 {
        return Err(HelperError::SystemCommandFailed(
            "no live system configuration was available for rollback".into(),
        ));
    }
    run_owned("tar", &args).await
}

async fn extract_rollback_archive(archive: &Path) -> Result<(), HelperError> {
    let rollback_text = archive.to_string_lossy().into_owned();
    run_command("tar", &["-xzf", &rollback_text, "-C", "/"]).await
}

async fn remove_managed_paths() -> Result<(), HelperError> {
    for relative in MANAGED_PATHS {
        let path = Path::new("/").join(relative);
        let Ok(metadata) = tokio::fs::symlink_metadata(&path).await else {
            continue;
        };
        if metadata.is_dir() {
            tokio::fs::remove_dir_all(&path)
                .await
                .map_err(system_error)?;
        } else {
            tokio::fs::remove_file(&path)
                .await
                .map_err(system_error)?;
        }
    }
    Ok(())
}

async fn validate_live_ufw_rules() -> Result<(), HelperError> {
    let ipv4 = Path::new("/etc/ufw/user.rules");
    if ipv4.exists() {
        run_with_stdin("iptables-restore", &["--test"], ipv4).await?;
    }
    let ipv6 = Path::new("/etc/ufw/user6.rules");
    if ipv6.exists() {
        run_with_stdin("ip6tables-restore", &["--test"], ipv6).await?;
    }
    Ok(())
}

async fn reload_ssh() -> Result<(), HelperError> {
    if run_command("systemctl", &["reload", "ssh.service"])
        .await
        .is_ok()
    {
        return Ok(());
    }
    run_command("systemctl", &["reload", "sshd.service"]).await
}

async fn verify_listeners(ports: &[u16]) -> Result<(), HelperError> {
    let output = run_capture("ss", &["-ltnH"]).await?;
    for port in ports {
        let suffix = format!(":{port}");
        let listening = output.lines().any(|line| {
            line.split_whitespace()
                .nth(3)
                .is_some_and(|address| address.ends_with(&suffix))
        });
        if !listening {
            return Err(HelperError::SystemCommandFailed(format!(
                "restored SSH port {port} is not listening"
            )));
        }
    }
    Ok(())
}

fn parse_sshd_ports(output: &str) -> Result<Vec<u16>, HelperError> {
    let mut ports = output
        .lines()
        .filter_map(|line| line.strip_prefix("port "))
        .filter_map(|value| value.trim().parse::<u16>().ok())
        .filter(|port| *port != 0)
        .collect::<Vec<_>>();
    ports.sort_unstable();
    ports.dedup();
    if ports.is_empty() {
        Err(HelperError::SystemCommandFailed(
            "could not determine restored SSH port".into(),
        ))
    } else {
        Ok(ports)
    }
}

async fn ufw_is_active() -> Result<bool, HelperError> {
    let output = run_capture("ufw", &["status"]).await?;
    Ok(output
        .lines()
        .next()
        .and_then(|line| line.strip_prefix("Status: "))
        .is_some_and(|value| value.trim().eq_ignore_ascii_case("active")))
}

async fn schedule_rollback(restore_id: &str) -> Result<(), HelperError> {
    let executable = std::env::current_exe().map_err(system_error)?;
    let executable = executable.to_string_lossy().into_owned();
    let unit = rollback_unit(restore_id);
    let restore_root = transaction_root().to_string_lossy().into_owned();
    let args = vec![
        "--unit".to_string(),
        unit,
        "--on-active".to_string(),
        ROLLBACK_SECONDS.to_string(),
        format!("--setenv=ARGUS_RESTORE_DIR={restore_root}"),
        executable,
        "--restore-rollback".to_string(),
        restore_id.to_string(),
    ];
    run_owned("systemd-run", &args).await
}

fn rollback_unit(restore_id: &str) -> String {
    format!("argus-restore-rollback-{restore_id}")
}

fn backup_dir() -> PathBuf {
    std::env::var_os("ARGUS_BACKUP_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/var/lib/argus/backups"))
}

fn transaction_root() -> PathBuf {
    std::env::var_os("ARGUS_RESTORE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            backup_dir()
                .parent()
                .unwrap_or_else(|| Path::new("/var/lib/argus"))
                .join("restores")
        })
}

fn transaction_dir(restore_id: &str) -> PathBuf {
    transaction_root().join(restore_id)
}

fn validate_restore_id(value: &str) -> Result<(), HelperError> {
    let valid = !value.is_empty()
        && value.len() <= 64
        && value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '-');
    if valid {
        Ok(())
    } else {
        Err(HelperError::InvalidRequest)
    }
}

async fn ensure_private_dir(path: &Path) -> Result<(), HelperError> {
    tokio::fs::create_dir_all(path)
        .await
        .map_err(system_error)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        tokio::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700))
            .await
            .map_err(system_error)?;
    }
    Ok(())
}

async fn run_command(program: &str, args: &[&str]) -> Result<(), HelperError> {
    run_capture(program, args).await.map(|_| ())
}

async fn run_owned(program: &str, args: &[String]) -> Result<(), HelperError> {
    let args = args.iter().map(String::as_str).collect::<Vec<_>>();
    run_command(program, &args).await
}

async fn run_capture(program: &str, args: &[&str]) -> Result<String, HelperError> {
    let output = Command::new(program)
        .args(args)
        .output()
        .await
        .map_err(|error| map_spawn(program, error))?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    } else {
        Err(HelperError::SystemCommandFailed(
            String::from_utf8_lossy(&output.stderr).trim().to_string(),
        ))
    }
}

async fn run_with_stdin(program: &str, args: &[&str], input: &Path) -> Result<(), HelperError> {
    let bytes = tokio::fs::read(input).await.map_err(system_error)?;
    let mut child = Command::new(program)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| map_spawn(program, error))?;
    child
        .stdin
        .as_mut()
        .ok_or_else(|| HelperError::SystemCommandFailed("validator stdin unavailable".into()))?
        .write_all(&bytes)
        .await
        .map_err(system_error)?;
    let output = child.wait_with_output().await.map_err(system_error)?;
    if output.status.success() {
        Ok(())
    } else {
        Err(HelperError::SystemCommandFailed(
            String::from_utf8_lossy(&output.stderr).trim().to_string(),
        ))
    }
}

fn map_spawn(program: &str, error: std::io::Error) -> HelperError {
    if error.kind() == std::io::ErrorKind::NotFound {
        HelperError::UtilityUnavailable(program.into())
    } else {
        HelperError::SystemCommandFailed(error.to_string())
    }
}

fn system_error(error: std::io::Error) -> HelperError {
    HelperError::SystemCommandFailed(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn restore_ids_cannot_escape_transaction_directory() {
        assert!(validate_restore_id("550e8400-e29b-41d4-a716-446655440000").is_ok());
        for invalid in ["", "../restore", "restore;reboot", "restore name"] {
            assert!(validate_restore_id(invalid).is_err());
        }
    }

    #[test]
    fn restored_ssh_ports_are_normalized() {
        let ports = parse_sshd_ports("port 2222\nport 22\nport 2222\n").unwrap();
        assert_eq!(ports, vec![22, 2222]);
    }
}
