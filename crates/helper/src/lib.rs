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

    pub async fn restart_service(&self, service: &str) -> Result<(), HelperError> {
        if !service.ends_with(".service") || service.contains(' ') {
            return Err(HelperError::InvalidServiceName);
        }

        if !self.allowlisted_services.contains(service) {
            return Err(HelperError::ServiceNotAllowlisted);
        }

        let output = Command::new("systemctl")
            .arg("restart")
            .arg(service)
            .output()
            .await
            .map_err(|e| HelperError::SystemCommandFailed(e.to_string()))?;

        if output.status.success() {
            Ok(())
        } else {
            Err(HelperError::SystemCommandFailed(
                String::from_utf8_lossy(&output.stderr).to_string(),
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn blocks_non_allowlisted_service() {
        let helper = HelperApi::from_allowlist(vec!["nginx.service".to_string()]);
        let error = helper
            .restart_service("docker.service")
            .await
            .expect_err("must reject");
        assert!(matches!(error, HelperError::ServiceNotAllowlisted));
    }

    #[tokio::test]
    async fn blocks_invalid_name() {
        let helper = HelperApi::from_allowlist(vec!["nginx.service".to_string()]);
        let error = helper
            .restart_service("../../etc/passwd")
            .await
            .expect_err("must reject");
        assert!(matches!(error, HelperError::InvalidServiceName));
    }
}
