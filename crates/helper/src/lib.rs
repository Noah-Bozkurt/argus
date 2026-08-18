use protocol::{SecurityFinding, SecurityState};
use std::collections::HashSet;
use thiserror::Error;
use tokio::process::Command;

const MAX_JOURNAL_LINES: u32 = 500;
const MAX_OUTPUT_BYTES: usize = 64 * 1024;
const MAX_DOCKER_OUTPUT_BYTES: usize = 256 * 1024;

#[derive(Debug, Error)]
pub enum HelperError {
    #[error("service not allowlisted")] ServiceNotAllowlisted,
    #[error("service name is invalid")] InvalidServiceName,
    #[error("container reference is invalid")] InvalidContainerReference,
    #[error("invalid request")] InvalidRequest,
    #[error("required system utility unavailable: {0}")] UtilityUnavailable(String),
    #[error("system command failed: {0}")] SystemCommandFailed(String),
}

#[derive(Debug, Clone)]
pub struct HelperApi { allowlisted_services: HashSet<String> }
impl HelperApi {
    pub fn from_allowlist(services: impl IntoIterator<Item = String>) -> Self { Self { allowlisted_services: services.into_iter().collect() } }
    pub fn from_env() -> Self {
        let values = std::env::var("ARGUS_ALLOWED_SERVICES").unwrap_or_else(|_| "nginx.service".into())
            .split(',').map(str::trim).filter(|s| !s.is_empty()).map(ToOwned::to_owned).collect::<Vec<_>>();
        Self::from_allowlist(values)
    }
    pub fn validate_service_name(service: &str) -> Result<(), HelperError> {
        let valid = service.ends_with(".service") && !service.is_empty() && service.len() <= 255
            && service.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '@'));
        if valid { Ok(()) } else { Err(HelperError::InvalidServiceName) }
    }
    pub fn validate_container_reference(container: &str) -> Result<(), HelperError> {
        let valid = !container.is_empty() && container.len() <= 128
            && container.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'));
        if valid { Ok(()) } else { Err(HelperError::InvalidContainerReference) }
    }
    fn ensure_allowlisted(&self, service: &str) -> Result<(), HelperError> {
        Self::validate_service_name(service)?;
        if self.allowlisted_services.contains(service) { Ok(()) } else { Err(HelperError::ServiceNotAllowlisted) }
    }
    pub async fn restart_service(&self, service: &str) -> Result<(), HelperError> { self.service_action("restart", service).await }
    pub async fn start_service(&self, service: &str) -> Result<(), HelperError> { self.service_action("start", service).await }
    pub async fn stop_service(&self, service: &str) -> Result<(), HelperError> { self.service_action("stop", service).await }
    async fn service_action(&self, action: &str, service: &str) -> Result<(), HelperError> { self.ensure_allowlisted(service)?; run("systemctl", &[action, service]).await }
    pub async fn journal(&self, service: &str, lines: u32) -> Result<String, HelperError> {
        self.ensure_allowlisted(service)?;
        if lines == 0 || lines > MAX_JOURNAL_LINES { return Err(HelperError::InvalidRequest); }
        let line_arg = lines.to_string();
        Ok(truncate_utf8(run_capture("journalctl", &["--no-pager", "--output=short-iso", "-u", service, "-n", &line_arg]).await?, MAX_OUTPUT_BYTES))
    }
    pub async fn docker_list(&self) -> Result<String, HelperError> {
        Ok(truncate_utf8(run_capture("docker", &["ps", "-a", "--no-trunc", "--format", "{{json .}}"]).await?, MAX_DOCKER_OUTPUT_BYTES))
    }
    pub async fn docker_start(&self, container: &str) -> Result<(), HelperError> { self.docker_action("start", container).await }
    pub async fn docker_stop(&self, container: &str) -> Result<(), HelperError> { self.docker_action("stop", container).await }
    pub async fn docker_restart(&self, container: &str) -> Result<(), HelperError> { self.docker_action("restart", container).await }
    async fn docker_action(&self, action: &str, container: &str) -> Result<(), HelperError> { Self::validate_container_reference(container)?; run("docker", &[action, container]).await }

    pub async fn security_inspect(&self) -> Result<SecurityState, HelperError> {
        let ssh = run_capture("sshd", &["-T"]).await.unwrap_or_default();
        let password_auth = ssh.lines().find_map(|line| line.strip_prefix("passwordauthentication ")).map(|v| v == "yes");
        let root_login = ssh.lines().find_map(|line| line.strip_prefix("permitrootlogin ")).unwrap_or("unknown").to_string();

        let ufw = run_capture("ufw", &["status", "numbered"]).await.unwrap_or_default();
        let firewall_status = ufw.lines().next().and_then(|line| line.strip_prefix("Status: ")).unwrap_or("unavailable").to_lowercase();
        let firewall_rules = ufw.lines().filter(|line| line.trim_start().starts_with('[')).take(100).map(|line| line.trim().to_string()).collect::<Vec<_>>();

        let auto_config = tokio::fs::read_to_string("/etc/apt/apt.conf.d/20auto-upgrades").await.unwrap_or_default();
        let automatic_security_updates = auto_config.contains("APT::Periodic::Unattended-Upgrade \"1\"");
        let mut findings = Vec::new();
        if password_auth == Some(true) { findings.push(finding("HIGH", "SSH_PASSWORD_AUTH", "SSH password authentication is enabled")); }
        if root_login == "yes" { findings.push(finding("HIGH", "SSH_ROOT_LOGIN", "Direct SSH root login is enabled")); }
        if firewall_status != "active" { findings.push(finding("MEDIUM", "FIREWALL_INACTIVE", "UFW is not active")); }
        if !automatic_security_updates { findings.push(finding("MEDIUM", "AUTO_SECURITY_UPDATES_DISABLED", "Automatic security updates are not enabled")); }
        Ok(SecurityState { available: !ssh.is_empty() || !ufw.is_empty(), firewall_status, firewall_rules, ssh_password_auth: password_auth, ssh_root_login: root_login, automatic_security_updates, findings })
    }

    pub async fn refresh_packages(&self) -> Result<(), HelperError> { run("apt-get", &["update"]).await }
    pub async fn upgrade_all_packages(&self) -> Result<(), HelperError> { run("apt-get", &["-y", "-o", "Dpkg::Options::=--force-confold", "upgrade"]).await }
    pub async fn upgrade_security_packages(&self) -> Result<(), HelperError> {
        if tokio::fs::metadata("/usr/bin/unattended-upgrade").await.is_err() { return Err(HelperError::UtilityUnavailable("unattended-upgrade".into())); }
        run("unattended-upgrade", &["--verbose"]).await
    }
    pub async fn reboot(&self) -> Result<(), HelperError> { run("systemctl", &["reboot"]).await }
}

fn finding(severity: &str, code: &str, message: &str) -> SecurityFinding { SecurityFinding { severity: severity.into(), code: code.into(), message: message.into() } }
async fn run(program: &str, args: &[&str]) -> Result<(), HelperError> { run_capture(program, args).await.map(|_| ()) }
async fn run_capture(program: &str, args: &[&str]) -> Result<String, HelperError> {
    let output = Command::new(program).args(args).env("DEBIAN_FRONTEND", "noninteractive").output().await.map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound { HelperError::UtilityUnavailable(program.into()) } else { HelperError::SystemCommandFailed(e.to_string()) }
    })?;
    if output.status.success() { Ok(String::from_utf8_lossy(&output.stdout).into_owned()) }
    else { Err(HelperError::SystemCommandFailed(String::from_utf8_lossy(&output.stderr).trim().to_string())) }
}
fn truncate_utf8(mut value: String, max_bytes: usize) -> String {
    if value.len() <= max_bytes { return value; }
    let mut boundary = max_bytes; while !value.is_char_boundary(boundary) { boundary -= 1; }
    value.truncate(boundary); value.push_str("\n[output truncated]\n"); value
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn blocks_shell_and_path_input() {
        for invalid in ["../../etc/passwd", "nginx.service;id", "nginx service", "nginx.service$(id)"] { assert!(matches!(HelperApi::validate_service_name(invalid), Err(HelperError::InvalidServiceName))); }
    }
    #[test]
    fn blocks_invalid_container_references() {
        for invalid in ["../web", "web;id", "web name", "$(id)", ""] { assert!(matches!(HelperApi::validate_container_reference(invalid), Err(HelperError::InvalidContainerReference))); }
    }
    #[test]
    fn finding_preserves_severity_and_code() {
        let finding = finding("HIGH", "TEST", "test"); assert_eq!(finding.severity, "HIGH"); assert_eq!(finding.code, "TEST");
    }
}
