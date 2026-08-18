use std::collections::HashSet;
use thiserror::Error;
use tokio::process::Command;

#[derive(Debug, Error)]
pub enum HelperError {
    #[error("service not allowlisted")]
    ServiceNotAllowlisted,
    #[error("service name is invalid")]
    InvalidServiceName,
    #[error("system command failed: {0}")]
    SystemCommandFailed(String),
}

#[derive(Debug, Clone)]
pub struct HelperApi {
    allowlisted_services: HashSet<String>,
}

impl HelperApi {
    pub fn from_allowlist(allowlisted_services: impl IntoIterator<Item = String>) -> Self {
        Self {
            allowlisted_services: allowlisted_services.into_iter().collect(),
        }
    }

    pub fn from_env() -> Self {
        let values = std::env::var("ARGUS_ALLOWED_SERVICES")
            .unwrap_or_else(|_| "nginx.service".to_string())
            .split(',')
            .map(str::trim)
            .filter(|item| !item.is_empty())
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
        Self::validate_service_name(service)?;
        if !self.allowlisted_services.contains(service) {
            return Err(HelperError::ServiceNotAllowlisted);
        }

        let output = Command::new("systemctl")
            .arg(action)
            .arg(service)
            .output()
            .await
            .map_err(|e| HelperError::SystemCommandFailed(e.to_string()))?;

        if output.status.success() {
            Ok(())
        } else {
            Err(HelperError::SystemCommandFailed(
                String::from_utf8_lossy(&output.stderr).trim().to_string(),
            ))
        }
    }
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
            helper.start_service("docker.service").await,
            Err(HelperError::ServiceNotAllowlisted)
        ));
        assert!(matches!(
            helper.stop_service("docker.service").await,
            Err(HelperError::ServiceNotAllowlisted)
        ));
    }
}
