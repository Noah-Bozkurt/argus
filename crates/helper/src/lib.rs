use std::collections::HashSet;
use thiserror::Error;
use tokio::process::Command;

const MAX_JOURNAL_LINES: u32 = 500;
const MAX_OUTPUT_BYTES: usize = 64 * 1024;

#[derive(Debug, Error)]
pub enum HelperError {
    #[error("service not allowlisted")]
    ServiceNotAllowlisted,
    #[error("service name is invalid")]
    InvalidServiceName,
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
}

impl HelperApi {
    pub fn from_allowlist(services: impl IntoIterator<Item = String>) -> Self {
        Self {
            allowlisted_services: services.into_iter().collect(),
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
        Self::from_allowlist(values)
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
        let output = run_capture(
            "journalctl",
            &["--no-pager", "--output=short-iso", "-u", service, "-n", &line_arg],
        )
        .await?;
        Ok(truncate_utf8(output, MAX_OUTPUT_BYTES))
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
    #[tokio::test]
    async fn blocks_non_allowlisted_service_before_execution() {
        let helper = HelperApi::from_allowlist(["nginx.service".to_string()]);
        assert!(matches!(
            helper.restart_service("docker.service").await,
            Err(HelperError::ServiceNotAllowlisted)
        ));
        assert!(matches!(
            helper.journal("docker.service", 100).await,
            Err(HelperError::ServiceNotAllowlisted)
        ));
    }
    #[tokio::test]
    async fn rejects_unbounded_journal_requests_before_execution() {
        let helper = HelperApi::from_allowlist(["nginx.service".to_string()]);
        assert!(matches!(
            helper.journal("nginx.service", 501).await,
            Err(HelperError::InvalidRequest)
        ));
    }
    #[test]
    fn output_truncation_is_bounded() {
        let output = truncate_utf8("x".repeat(100), 10);
        assert!(output.starts_with("xxxxxxxxxx"));
        assert!(output.contains("truncated"));
    }
}
