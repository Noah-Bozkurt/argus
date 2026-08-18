use protocol::{BackupArtifact, BackupState, SecurityFinding, SecurityState};
use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};
use thiserror::Error;
use tokio::process::Command;

const MAX_JOURNAL_LINES: u32 = 500;
const MAX_OUTPUT_BYTES: usize = 64 * 1024;
const MAX_DOCKER_OUTPUT_BYTES: usize = 256 * 1024;
const MAX_BACKUPS: usize = 50;

#[derive(Debug, Error)]
pub enum HelperError {
    #[error("service not allowlisted")]
    ServiceNotAllowlisted,
    #[error("service name is invalid")]
    InvalidServiceName,
    #[error("container reference is invalid")]
    InvalidContainerReference,
    #[error("backup reference is invalid")]
    InvalidBackupReference,
    #[error("backup integrity verification failed")]
    BackupIntegrityFailed,
    #[error("invalid request")]
    InvalidRequest,
    #[error("required system utility unavailable: {0}")]
    UtilityUnavailable(String),
    #[error("system command failed: {0}")]
    SystemCommandFailed(String),
}

#[derive(Debug, Clone)]
pub struct HelperApi {
    allowlisted_services: HashSet<String>,
    backup_dir: PathBuf,
}
impl HelperApi {
    pub fn from_allowlist(services: impl IntoIterator<Item = String>) -> Self {
        Self {
            allowlisted_services: services.into_iter().collect(),
            backup_dir: PathBuf::from("/var/lib/argus/backups"),
        }
    }
    pub fn from_env() -> Self {
        let values = std::env::var("ARGUS_ALLOWED_SERVICES")
            .unwrap_or_else(|_| "nginx.service".into())
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(ToOwned::to_owned)
            .collect::<Vec<_>>();
        let mut api = Self::from_allowlist(values);
        api.backup_dir = std::env::var_os("ARGUS_BACKUP_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("/var/lib/argus/backups"));
        api
    }
    pub fn validate_service_name(service: &str) -> Result<(), HelperError> {
        let valid = service.ends_with(".service")
            && !service.is_empty()
            && service.len() <= 255
            && service
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '@'));
        if valid {
            Ok(())
        } else {
            Err(HelperError::InvalidServiceName)
        }
    }
    pub fn validate_container_reference(container: &str) -> Result<(), HelperError> {
        let valid = !container.is_empty()
            && container.len() <= 128
            && container
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'));
        if valid {
            Ok(())
        } else {
            Err(HelperError::InvalidContainerReference)
        }
    }
    pub fn validate_backup_reference(backup: &str) -> Result<(), HelperError> {
        let valid = backup.ends_with(".tar.gz")
            && backup.len() <= 160
            && backup
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'));
        if valid {
            Ok(())
        } else {
            Err(HelperError::InvalidBackupReference)
        }
    }
    fn ensure_allowlisted(&self, service: &str) -> Result<(), HelperError> {
        Self::validate_service_name(service)?;
        if self.allowlisted_services.contains(service) {
            Ok(())
        } else {
            Err(HelperError::ServiceNotAllowlisted)
        }
    }
    pub async fn restart_service(&self, service: &str) -> Result<(), HelperError> {
        self.service_action("restart", service).await
    }
    pub async fn start_service(&self, service: &str) -> Result<(), HelperError> {
        self.service_action("start", service).await
    }
    pub async fn stop_service(&self, service: &str) -> Result<(), HelperError> {
        self.service_action("stop", service).await
    }
    async fn service_action(&self, action: &str, service: &str) -> Result<(), HelperError> {
        self.ensure_allowlisted(service)?;
        run("systemctl", &[action, service]).await
    }
    pub async fn journal(&self, service: &str, lines: u32) -> Result<String, HelperError> {
        self.ensure_allowlisted(service)?;
        if lines == 0 || lines > MAX_JOURNAL_LINES {
            return Err(HelperError::InvalidRequest);
        }
        let line_arg = lines.to_string();
        Ok(truncate_utf8(
            run_capture(
                "journalctl",
                &[
                    "--no-pager",
                    "--output=short-iso",
                    "-u",
                    service,
                    "-n",
                    &line_arg,
                ],
            )
            .await?,
            MAX_OUTPUT_BYTES,
        ))
    }
    pub async fn docker_list(&self) -> Result<String, HelperError> {
        Ok(truncate_utf8(
            run_capture(
                "docker",
                &["ps", "-a", "--no-trunc", "--format", "{{json .}}"],
            )
            .await?,
            MAX_DOCKER_OUTPUT_BYTES,
        ))
    }
    pub async fn docker_start(&self, container: &str) -> Result<(), HelperError> {
        self.docker_action("start", container).await
    }
    pub async fn docker_stop(&self, container: &str) -> Result<(), HelperError> {
        self.docker_action("stop", container).await
    }
    pub async fn docker_restart(&self, container: &str) -> Result<(), HelperError> {
        self.docker_action("restart", container).await
    }
    async fn docker_action(&self, action: &str, container: &str) -> Result<(), HelperError> {
        Self::validate_container_reference(container)?;
        run("docker", &[action, container]).await
    }

    pub async fn security_inspect(&self) -> Result<SecurityState, HelperError> {
        let ssh = run_capture("sshd", &["-T"]).await.unwrap_or_default();
        let password_auth = ssh
            .lines()
            .find_map(|line| line.strip_prefix("passwordauthentication "))
            .map(|v| v == "yes");
        let root_login = ssh
            .lines()
            .find_map(|line| line.strip_prefix("permitrootlogin "))
            .unwrap_or("unknown")
            .to_string();

        let ufw = run_capture("ufw", &["status", "numbered"])
            .await
            .unwrap_or_default();
        let firewall_status = ufw
            .lines()
            .next()
            .and_then(|line| line.strip_prefix("Status: "))
            .unwrap_or("unavailable")
            .to_lowercase();
        let firewall_rules = ufw
            .lines()
            .filter(|line| line.trim_start().starts_with('['))
            .take(100)
            .map(|line| line.trim().to_string())
            .collect::<Vec<_>>();

        let auto_config = tokio::fs::read_to_string("/etc/apt/apt.conf.d/20auto-upgrades")
            .await
            .unwrap_or_default();
        let automatic_security_updates =
            auto_config.contains("APT::Periodic::Unattended-Upgrade \"1\"");
        let mut findings = Vec::new();
        if password_auth == Some(true) {
            findings.push(finding(
                "HIGH",
                "SSH_PASSWORD_AUTH",
                "SSH password authentication is enabled",
            ));
        }
        if root_login == "yes" {
            findings.push(finding(
                "HIGH",
                "SSH_ROOT_LOGIN",
                "Direct SSH root login is enabled",
            ));
        }
        if firewall_status != "active" {
            findings.push(finding("MEDIUM", "FIREWALL_INACTIVE", "UFW is not active"));
        }
        if !automatic_security_updates {
            findings.push(finding(
                "MEDIUM",
                "AUTO_SECURITY_UPDATES_DISABLED",
                "Automatic security updates are not enabled",
            ));
        }
        Ok(SecurityState {
            available: !ssh.is_empty() || !ufw.is_empty(),
            firewall_status,
            firewall_rules,
            ssh_password_auth: password_auth,
            ssh_root_login: root_login,
            automatic_security_updates,
            findings,
        })
    }

    async fn ensure_backup_dir(&self) -> Result<(), HelperError> {
        tokio::fs::create_dir_all(&self.backup_dir)
            .await
            .map_err(|e| HelperError::SystemCommandFailed(e.to_string()))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            tokio::fs::set_permissions(&self.backup_dir, std::fs::Permissions::from_mode(0o700))
                .await
                .map_err(|e| HelperError::SystemCommandFailed(e.to_string()))?;
        }
        Ok(())
    }

    pub async fn backup_create(&self, backup_id: &str, profile: &str) -> Result<(), HelperError> {
        if profile != "system-config"
            || backup_id.is_empty()
            || backup_id.len() > 64
            || !backup_id
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-')
        {
            return Err(HelperError::InvalidRequest);
        }
        self.ensure_backup_dir().await?;
        let name = format!("{backup_id}.tar.gz");
        let archive = self.backup_dir.join(&name);
        let archive_text = archive.to_string_lossy().into_owned();
        run(
            "tar",
            &[
                "-C",
                "/",
                "--ignore-failed-read",
                "-czf",
                &archive_text,
                "etc/ssh/sshd_config",
                "etc/ssh/sshd_config.d",
                "etc/ufw",
                "etc/apt/apt.conf.d/20auto-upgrades",
            ],
        )
        .await?;
        let sha = sha256_file(&archive).await?;
        tokio::fs::write(self.backup_dir.join(format!("{name}.sha256")), format!("{sha}\n"))
            .await
            .map_err(|e| HelperError::SystemCommandFailed(e.to_string()))?;
        let _ = tokio::fs::remove_file(self.backup_dir.join(format!("{name}.verified"))).await;
        Ok(())
    }

    pub async fn backup_verify(&self, backup: &str) -> Result<(), HelperError> {
        Self::validate_backup_reference(backup)?;
        self.ensure_backup_dir().await?;
        let archive = self.backup_dir.join(backup);
        let expected = tokio::fs::read_to_string(self.backup_dir.join(format!("{backup}.sha256")))
            .await
            .map_err(|_| HelperError::BackupIntegrityFailed)?;
        let expected = expected.trim();
        let actual = sha256_file(&archive).await?;
        if expected.is_empty() || actual != expected {
            return Err(HelperError::BackupIntegrityFailed);
        }
        let archive_text = archive.to_string_lossy().into_owned();
        run("tar", &["-tzf", &archive_text]).await?;
        tokio::fs::write(
            self.backup_dir.join(format!("{backup}.verified")),
            format!("{actual}\n"),
        )
        .await
        .map_err(|e| HelperError::SystemCommandFailed(e.to_string()))?;
        Ok(())
    }

    pub async fn backup_list(&self) -> Result<BackupState, HelperError> {
        self.ensure_backup_dir().await?;
        let mut entries = tokio::fs::read_dir(&self.backup_dir)
            .await
            .map_err(|e| HelperError::SystemCommandFailed(e.to_string()))?;
        let mut artifacts = Vec::new();
        while let Some(entry) = entries
            .next_entry()
            .await
            .map_err(|e| HelperError::SystemCommandFailed(e.to_string()))?
        {
            if artifacts.len() >= MAX_BACKUPS {
                break;
            }
            let name = entry.file_name().to_string_lossy().into_owned();
            if !name.ends_with(".tar.gz") || Self::validate_backup_reference(&name).is_err() {
                continue;
            }
            let metadata = entry
                .metadata()
                .await
                .map_err(|e| HelperError::SystemCommandFailed(e.to_string()))?;
            if !metadata.is_file() {
                continue;
            }
            let created_unix = metadata
                .modified()
                .unwrap_or(SystemTime::UNIX_EPOCH)
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            let expected = tokio::fs::read_to_string(self.backup_dir.join(format!("{name}.sha256")))
                .await
                .unwrap_or_default()
                .trim()
                .to_string();
            let verified_marker = tokio::fs::read_to_string(
                self.backup_dir.join(format!("{name}.verified")),
            )
            .await
            .unwrap_or_default();
            let verified = !expected.is_empty() && verified_marker.trim() == expected;
            artifacts.push(BackupArtifact {
                name,
                profile: "system-config".into(),
                size_bytes: metadata.len(),
                created_unix,
                sha256: expected,
                verified,
            });
        }
        artifacts.sort_by(|a, b| b.created_unix.cmp(&a.created_unix));
        Ok(BackupState {
            available: true,
            target: self.backup_dir.display().to_string(),
            artifacts,
        })
    }

    pub async fn refresh_packages(&self) -> Result<(), HelperError> {
        run("apt-get", &["update"]).await
    }
    pub async fn upgrade_all_packages(&self) -> Result<(), HelperError> {
        run(
            "apt-get",
            &["-y", "-o", "Dpkg::Options::=--force-confold", "upgrade"],
        )
        .await
    }
    pub async fn upgrade_security_packages(&self) -> Result<(), HelperError> {
        if tokio::fs::metadata("/usr/bin/unattended-upgrade")
            .await
            .is_err()
        {
            return Err(HelperError::UtilityUnavailable("unattended-upgrade".into()));
        }
        run("unattended-upgrade", &["--verbose"]).await
    }
    pub async fn reboot(&self) -> Result<(), HelperError> {
        run("systemctl", &["reboot"]).await
    }
}

fn finding(severity: &str, code: &str, message: &str) -> SecurityFinding {
    SecurityFinding {
        severity: severity.into(),
        code: code.into(),
        message: message.into(),
    }
}
async fn sha256_file(path: &Path) -> Result<String, HelperError> {
    let value = path.to_string_lossy().into_owned();
    let output = run_capture("sha256sum", &[&value]).await?;
    output
        .split_whitespace()
        .next()
        .filter(|hash| hash.len() == 64 && hash.chars().all(|c| c.is_ascii_hexdigit()))
        .map(str::to_lowercase)
        .ok_or_else(|| HelperError::SystemCommandFailed("invalid sha256sum output".into()))
}
async fn run(program: &str, args: &[&str]) -> Result<(), HelperError> {
    run_capture(program, args).await.map(|_| ())
}
async fn run_capture(program: &str, args: &[&str]) -> Result<String, HelperError> {
    let output = Command::new(program)
        .args(args)
        .env("DEBIAN_FRONTEND", "noninteractive")
        .output()
        .await
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                HelperError::UtilityUnavailable(program.into())
            } else {
                HelperError::SystemCommandFailed(e.to_string())
            }
        })?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    } else {
        Err(HelperError::SystemCommandFailed(
            String::from_utf8_lossy(&output.stderr).trim().to_string(),
        ))
    }
}
fn truncate_utf8(mut value: String, max_bytes: usize) -> String {
    if value.len() <= max_bytes {
        return value;
    }
    let mut boundary = max_bytes;
    while !value.is_char_boundary(boundary) {
        boundary -= 1;
    }
    value.truncate(boundary);
    value.push_str("\n[output truncated]\n");
    value
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn blocks_shell_and_path_input() {
        for invalid in [
            "../../etc/passwd",
            "nginx.service;id",
            "nginx service",
            "nginx.service$(id)",
        ] {
            assert!(matches!(
                HelperApi::validate_service_name(invalid),
                Err(HelperError::InvalidServiceName)
            ));
        }
    }
    #[test]
    fn blocks_invalid_container_references() {
        for invalid in ["../web", "web;id", "web name", "$(id)", ""] {
            assert!(matches!(
                HelperApi::validate_container_reference(invalid),
                Err(HelperError::InvalidContainerReference)
            ));
        }
    }
    #[test]
    fn blocks_backup_path_traversal() {
        for invalid in ["../backup.tar.gz", "/tmp/backup.tar.gz", "backup;id.tar.gz"] {
            assert!(matches!(
                HelperApi::validate_backup_reference(invalid),
                Err(HelperError::InvalidBackupReference)
            ));
        }
        assert!(HelperApi::validate_backup_reference("abc-123.tar.gz").is_ok());
    }
    #[test]
    fn finding_preserves_severity_and_code() {
        let finding = finding("HIGH", "TEST", "test");
        assert_eq!(finding.severity, "HIGH");
        assert_eq!(finding.code, "TEST");
    }
}
