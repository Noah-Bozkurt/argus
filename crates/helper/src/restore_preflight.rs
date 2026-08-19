use helper::HelperError;
use std::{
    path::{Component, Path, PathBuf},
    process::Stdio,
};
use tokio::{io::AsyncWriteExt, process::Command};

const MAX_ARCHIVE_ENTRIES: usize = 512;

pub async fn run(restore_id: &str, backup: &str) -> Result<String, HelperError> {
    validate_restore_id(restore_id)?;
    validate_backup_reference(backup)?;
    let backup_dir = backup_dir();
    let archive = backup_dir.join(backup);
    verify_integrity(&backup_dir, &archive, backup).await?;

    let entries = archive_entries(&archive).await?;
    validate_archive_entries(&entries)?;

    let restore_root = restore_root(&backup_dir);
    ensure_private_dir(&restore_root).await?;
    let staging = restore_root.join(restore_id);
    let _ = tokio::fs::remove_dir_all(&staging).await;
    ensure_private_dir(&staging).await?;

    let result = async {
        extract_candidate(&archive, &staging).await?;
        let mut checks = vec![
            "sha256".to_string(),
            "archive_allowlist".to_string(),
            "staged_extract".to_string(),
        ];
        validate_ssh(&staging).await?;
        checks.push("ssh_config".into());
        validate_apt(&staging).await?;
        checks.push("apt_config".into());
        validate_ufw(&staging, &mut checks).await?;
        serde_json::to_string(&serde_json::json!({
            "backup": backup,
            "checks": checks,
            "staged": true,
            "live_changes": false,
        }))
        .map_err(|error| HelperError::SystemCommandFailed(error.to_string()))
    }
    .await;

    let _ = tokio::fs::remove_dir_all(&staging).await;
    result
}

fn backup_dir() -> PathBuf {
    std::env::var_os("ARGUS_BACKUP_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/var/lib/argus/backups"))
}

fn restore_root(backup_dir: &Path) -> PathBuf {
    std::env::var_os("ARGUS_RESTORE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            backup_dir
                .parent()
                .unwrap_or_else(|| Path::new("/var/lib/argus"))
                .join("restores")
        })
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

fn validate_backup_reference(backup: &str) -> Result<(), HelperError> {
    let valid = backup.ends_with(".tar.gz")
        && backup.len() <= 160
        && backup
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.'));
    if valid {
        Ok(())
    } else {
        Err(HelperError::InvalidBackupReference)
    }
}

async fn ensure_private_dir(path: &Path) -> Result<(), HelperError> {
    tokio::fs::create_dir_all(path).await.map_err(system_error)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        tokio::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700))
            .await
            .map_err(system_error)?;
    }
    Ok(())
}

async fn verify_integrity(
    backup_dir: &Path,
    archive: &Path,
    backup: &str,
) -> Result<(), HelperError> {
    let expected = tokio::fs::read_to_string(backup_dir.join(format!("{backup}.sha256")))
        .await
        .map_err(|_| HelperError::BackupIntegrityFailed)?;
    let expected = expected.trim();
    if expected.len() != 64
        || !expected
            .chars()
            .all(|character| character.is_ascii_hexdigit())
    {
        return Err(HelperError::BackupIntegrityFailed);
    }
    let archive_text = archive.to_string_lossy().into_owned();
    let output = run_capture("sha256sum", &[&archive_text]).await?;
    let actual = output.split_whitespace().next().unwrap_or_default();
    if !actual.eq_ignore_ascii_case(expected) {
        return Err(HelperError::BackupIntegrityFailed);
    }
    Ok(())
}

#[derive(Debug)]
struct ArchiveEntry {
    kind: char,
    path: String,
}

async fn archive_entries(archive: &Path) -> Result<Vec<ArchiveEntry>, HelperError> {
    let archive_text = archive.to_string_lossy().into_owned();
    let output = run_capture("tar", &["-tvzf", &archive_text]).await?;
    let mut entries = Vec::new();
    for line in output.lines() {
        if entries.len() >= MAX_ARCHIVE_ENTRIES {
            return Err(HelperError::InvalidRequest);
        }
        let kind = line.chars().next().ok_or(HelperError::InvalidRequest)?;
        let path = line
            .split_whitespace()
            .nth(5)
            .ok_or(HelperError::InvalidRequest)?
            .to_string();
        entries.push(ArchiveEntry { kind, path });
    }
    if entries.is_empty() {
        return Err(HelperError::InvalidRequest);
    }
    Ok(entries)
}

fn validate_archive_entries(entries: &[ArchiveEntry]) -> Result<(), HelperError> {
    let mut ssh_config = false;
    let mut apt_config = false;
    for entry in entries {
        if !matches!(entry.kind, '-' | 'd')
            || !safe_relative_path(&entry.path)
            || !allowed_path(&entry.path)
        {
            return Err(HelperError::InvalidRequest);
        }
        ssh_config |= entry.path == "etc/ssh/sshd_config";
        apt_config |= entry.path == "etc/apt/apt.conf.d/20auto-upgrades";
    }
    if !ssh_config || !apt_config {
        return Err(HelperError::InvalidRequest);
    }
    Ok(())
}

fn safe_relative_path(value: &str) -> bool {
    let path = Path::new(value);
    !path.is_absolute()
        && path
            .components()
            .all(|component| matches!(component, Component::Normal(_) | Component::CurDir))
}

fn allowed_path(value: &str) -> bool {
    value == "etc/ssh/sshd_config"
        || value == "etc/apt/apt.conf.d/20auto-upgrades"
        || value == "etc/ssh/sshd_config.d"
        || value.starts_with("etc/ssh/sshd_config.d/")
        || value == "etc/ufw"
        || value.starts_with("etc/ufw/")
}

async fn extract_candidate(archive: &Path, staging: &Path) -> Result<(), HelperError> {
    let archive_text = archive.to_string_lossy().into_owned();
    let staging_text = staging.to_string_lossy().into_owned();
    run(
        "tar",
        &[
            "-xzf",
            &archive_text,
            "-C",
            &staging_text,
            "--no-same-owner",
            "--no-same-permissions",
        ],
    )
    .await
}

async fn validate_ssh(staging: &Path) -> Result<(), HelperError> {
    let source = staging.join("etc/ssh/sshd_config");
    let content = tokio::fs::read_to_string(&source)
        .await
        .map_err(system_error)?;
    let include_root = staging.join("etc/ssh/sshd_config.d");
    let include_root = include_root.to_string_lossy();
    let mut validation = String::new();
    for line in content.lines() {
        let trimmed = line.trim_start();
        if let Some(patterns) = trimmed.strip_prefix("Include ") {
            let mut rewritten = Vec::new();
            for pattern in patterns.split_whitespace() {
                let suffix = pattern
                    .strip_prefix("/etc/ssh/sshd_config.d/")
                    .ok_or(HelperError::InvalidRequest)?;
                rewritten.push(format!("{include_root}/{suffix}"));
            }
            let indent_len = line.len() - trimmed.len();
            validation.push_str(&line[..indent_len]);
            validation.push_str("Include ");
            validation.push_str(&rewritten.join(" "));
            validation.push('\n');
        } else {
            validation.push_str(line);
            validation.push('\n');
        }
    }
    let validation_path = staging.join("sshd_config.validation");
    tokio::fs::write(&validation_path, validation)
        .await
        .map_err(system_error)?;
    let validation_text = validation_path.to_string_lossy().into_owned();
    run("sshd", &["-t", "-f", &validation_text]).await
}

async fn validate_apt(staging: &Path) -> Result<(), HelperError> {
    let config = staging.join("etc/apt/apt.conf.d/20auto-upgrades");
    let config_text = config.to_string_lossy().into_owned();
    run("apt-config", &["-c", &config_text, "dump"]).await
}

async fn validate_ufw(staging: &Path, checks: &mut Vec<String>) -> Result<(), HelperError> {
    let ipv4 = staging.join("etc/ufw/user.rules");
    if ipv4.exists() {
        run_with_stdin("iptables-restore", &["--test"], &ipv4).await?;
        checks.push("ufw_ipv4_rules".into());
    }
    let ipv6 = staging.join("etc/ufw/user6.rules");
    if ipv6.exists() {
        run_with_stdin("ip6tables-restore", &["--test"], &ipv6).await?;
        checks.push("ufw_ipv6_rules".into());
    }
    Ok(())
}

async fn run(program: &str, args: &[&str]) -> Result<(), HelperError> {
    run_capture(program, args).await.map(|_| ())
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
    fn allowlist_rejects_traversal_and_unmanaged_paths() {
        assert!(safe_relative_path("etc/ssh/sshd_config"));
        assert!(!safe_relative_path("../etc/ssh/sshd_config"));
        assert!(!safe_relative_path("/etc/ssh/sshd_config"));
        assert!(allowed_path("etc/ufw/user.rules"));
        assert!(!allowed_path("etc/shadow"));
    }

    #[test]
    fn restore_ids_are_bounded_identifiers() {
        assert!(validate_restore_id("550e8400-e29b-41d4-a716-446655440000").is_ok());
        assert!(validate_restore_id("../restore").is_err());
        assert!(validate_restore_id("restore;id").is_err());
    }
}
