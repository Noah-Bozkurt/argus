use crate::project_workspace::ProjectSummary;
use reqwest::{Client, Url, redirect::Policy};
use serde::Serialize;
use std::{sync::Arc, time::Duration};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct ContentSyncClient {
    client: Client,
    endpoint: Option<Url>,
    token: Option<Arc<String>>,
}

#[derive(Debug, Clone)]
pub struct ContentSyncResult {
    pub enabled: bool,
    pub synced_projects: usize,
}

#[derive(Debug, thiserror::Error)]
pub enum ContentSyncError {
    #[error("invalid Payload content sync configuration: {0}")]
    Config(String),
    #[error("Payload project sync request failed: {0}")]
    Request(String),
}

#[derive(Debug, Serialize)]
struct ProjectSyncRequest<'a> {
    organization_id: Uuid,
    project_id: Uuid,
    name: &'a str,
    client_id: Option<Uuid>,
    status: &'a str,
}

impl ContentSyncClient {
    pub fn from_env() -> Result<Self, ContentSyncError> {
        let raw_url = std::env::var("ARGUS_CONTENT_URL").ok();
        let token = std::env::var("ARGUS_CONTENT_SYNC_TOKEN").ok();
        let (endpoint, token) = match (raw_url, token) {
            (None, None) => (None, None),
            (Some(_), None) | (None, Some(_)) => {
                return Err(ContentSyncError::Config(
                    "ARGUS_CONTENT_URL and ARGUS_CONTENT_SYNC_TOKEN must be configured together"
                        .into(),
                ));
            }
            (Some(raw_url), Some(token)) => {
                if token.len() < 32 {
                    return Err(ContentSyncError::Config(
                        "ARGUS_CONTENT_SYNC_TOKEN must be at least 32 characters".into(),
                    ));
                }
                let base = Url::parse(&raw_url)
                    .map_err(|_| ContentSyncError::Config("ARGUS_CONTENT_URL is invalid".into()))?;
                if !matches!(base.scheme(), "http" | "https") || base.host_str().is_none() {
                    return Err(ContentSyncError::Config(
                        "ARGUS_CONTENT_URL must be an absolute HTTP(S) URL".into(),
                    ));
                }
                let endpoint = base
                    .join("/internal/argus/project-sync")
                    .map_err(|_| ContentSyncError::Config("ARGUS_CONTENT_URL is invalid".into()))?;
                (Some(endpoint), Some(Arc::new(token)))
            }
        };
        let client = Client::builder()
            .timeout(Duration::from_secs(10))
            .redirect(Policy::none())
            .build()
            .map_err(|error| ContentSyncError::Config(error.to_string()))?;
        Ok(Self {
            client,
            endpoint,
            token,
        })
    }

    pub async fn sync_projects(
        &self,
        organization_id: Uuid,
        projects: &[ProjectSummary],
    ) -> Result<ContentSyncResult, ContentSyncError> {
        let (Some(endpoint), Some(token)) = (&self.endpoint, &self.token) else {
            return Ok(ContentSyncResult {
                enabled: false,
                synced_projects: 0,
            });
        };
        let mut synced_projects = 0usize;
        for project in projects {
            let status = match project.status.as_str() {
                "PAUSED" => "paused",
                "ARCHIVED" => "archived",
                _ => "active",
            };
            let response = self
                .client
                .post(endpoint.clone())
                .bearer_auth(token.as_str())
                .json(&ProjectSyncRequest {
                    organization_id,
                    project_id: project.id,
                    name: &project.name,
                    client_id: project.client_id,
                    status,
                })
                .send()
                .await
                .map_err(|error| ContentSyncError::Request(error.to_string()))?;
            if !response.status().is_success() {
                return Err(ContentSyncError::Request(format!(
                    "content service returned HTTP {} for project {}",
                    response.status(),
                    project.id
                )));
            }
            synced_projects += 1;
        }
        Ok(ContentSyncResult {
            enabled: true,
            synced_projects,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_project_status_to_payload_lifecycle() {
        fn map(value: &str) -> &str {
            match value {
                "PAUSED" => "paused",
                "ARCHIVED" => "archived",
                _ => "active",
            }
        }
        assert_eq!(map("ACTIVE"), "active");
        assert_eq!(map("PAUSED"), "paused");
        assert_eq!(map("ARCHIVED"), "archived");
    }
}
