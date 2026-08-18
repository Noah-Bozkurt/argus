use crate::{ApiError, AppState, api_error, web_identity};
use axum::{
    Json, Router,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    routing::get,
};
use chrono::{DateTime, Duration, Utc};
use serde::Serialize;
use sqlx::{PgPool, Row};
use std::collections::HashSet;
use uuid::Uuid;

const CORRELATION_WINDOW_MINUTES: i64 = 120;

#[derive(Debug, Clone)]
pub struct ChangeCorrelationStore {
    pool: PgPool,
}

#[derive(Debug, Clone, Serialize)]
pub struct CorrelatedChange {
    pub category: String,
    pub event_type: String,
    pub occurred_at: DateTime<Utc>,
    pub minutes_from_incident: i64,
    pub impact_related: bool,
    pub resource_type: Option<String>,
    pub resource_id: Option<Uuid>,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct CorrelationView {
    pub incident_id: Uuid,
    pub incident_started_at: DateTime<Utc>,
    pub window_minutes: i64,
    pub changes: Vec<CorrelatedChange>,
}

#[derive(Debug, thiserror::Error)]
pub enum ChangeCorrelationError {
    #[error("incident not found")]
    NotFound,
    #[error(transparent)]
    Sql(#[from] sqlx::Error),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct ResourceKey {
    resource_type: String,
    resource_id: Uuid,
}

impl ResourceKey {
    fn new(resource_type: impl Into<String>, resource_id: Uuid) -> Self {
        Self {
            resource_type: resource_type.into(),
            resource_id,
        }
    }
}

impl ChangeCorrelationStore {
    pub async fn connect(database_url: &str) -> Result<Self, sqlx::Error> {
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(10)
            .connect(database_url)
            .await?;
        Ok(Self { pool })
    }

    pub async fn incident_view(
        &self,
        organization_id: Uuid,
        project_id: Uuid,
        incident_id: Uuid,
    ) -> Result<CorrelationView, ChangeCorrelationError> {
        let incident = sqlx::query(
            "SELECT started_at,source_type,source_id FROM incidents WHERE id=$1 AND organization_id=$2 AND project_id=$3",
        )
        .bind(incident_id)
        .bind(organization_id)
        .bind(project_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(ChangeCorrelationError::NotFound)?;
        let started_at: DateTime<Utc> = incident.get("started_at");
        let mut impacted = HashSet::new();
        impacted.insert(ResourceKey::new(
            incident.get::<String, _>("source_type"),
            incident.get::<Uuid, _>("source_id"),
        ));
        let affected_rows = sqlx::query(
            "SELECT resource_type,resource_id FROM incident_affected_resources WHERE incident_id=$1 AND organization_id=$2 AND project_id=$3",
        )
        .bind(incident_id)
        .bind(organization_id)
        .bind(project_id)
        .fetch_all(&self.pool)
        .await?;
        for row in affected_rows {
            impacted.insert(ResourceKey::new(
                row.get::<String, _>("resource_type"),
                row.get::<Uuid, _>("resource_id"),
            ));
        }

        let from = started_at - Duration::minutes(CORRELATION_WINDOW_MINUTES);
        let to = started_at + Duration::minutes(CORRELATION_WINDOW_MINUTES);
        let mut changes = Vec::new();
        self.load_deployments(
            organization_id,
            project_id,
            started_at,
            from,
            to,
            &impacted,
            &mut changes,
        )
        .await?;
        self.load_releases(
            organization_id,
            project_id,
            started_at,
            from,
            to,
            &impacted,
            &mut changes,
        )
        .await?;
        self.load_commands(
            organization_id,
            project_id,
            started_at,
            from,
            to,
            &impacted,
            &mut changes,
        )
        .await?;
        self.load_project_events(
            organization_id,
            project_id,
            started_at,
            from,
            to,
            &impacted,
            &mut changes,
        )
        .await?;

        changes.sort_by(|left, right| {
            right
                .impact_related
                .cmp(&left.impact_related)
                .then_with(|| {
                    left.minutes_from_incident
                        .abs()
                        .cmp(&right.minutes_from_incident.abs())
                })
                .then_with(|| left.occurred_at.cmp(&right.occurred_at))
        });
        Ok(CorrelationView {
            incident_id,
            incident_started_at: started_at,
            window_minutes: CORRELATION_WINDOW_MINUTES,
            changes,
        })
    }

    #[allow(clippy::too_many_arguments)]
    async fn load_deployments(
        &self,
        organization_id: Uuid,
        project_id: Uuid,
        incident_started_at: DateTime<Utc>,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
        impacted: &HashSet<ResourceKey>,
        changes: &mut Vec<CorrelatedChange>,
    ) -> Result<(), sqlx::Error> {
        let rows = sqlx::query(
            "SELECT d.id,d.service_id,d.environment_id,d.repository_id,d.status,d.source_version,d.source_commit_sha,d.provider,COALESCE(d.started_at,d.created_at) AS occurred_at,s.name AS service_name,e.name AS environment_name FROM deployments d JOIN services s ON s.id=d.service_id JOIN environments e ON e.id=d.environment_id WHERE d.organization_id=$1 AND d.project_id=$2 AND COALESCE(d.started_at,d.created_at) BETWEEN $3 AND $4 ORDER BY occurred_at",
        )
        .bind(organization_id)
        .bind(project_id)
        .bind(from)
        .bind(to)
        .fetch_all(&self.pool)
        .await?;
        for row in rows {
            let service_id: Uuid = row.get("service_id");
            let environment_id: Uuid = row.get("environment_id");
            let repository_id: Option<Uuid> = row.get("repository_id");
            let occurred_at: DateTime<Utc> = row.get("occurred_at");
            let related = impacted.contains(&ResourceKey::new("SERVICE", service_id))
                || impacted.contains(&ResourceKey::new("ENVIRONMENT", environment_id))
                || repository_id
                    .is_some_and(|id| impacted.contains(&ResourceKey::new("REPOSITORY", id)));
            let service_name: String = row.get("service_name");
            let environment_name: String = row.get("environment_name");
            let status: String = row.get("status");
            let source_version: Option<String> = row.get("source_version");
            let source_commit_sha: Option<String> = row.get("source_commit_sha");
            let source = source_version
                .or_else(|| source_commit_sha.map(|sha| sha.chars().take(12).collect()))
                .unwrap_or_else(|| "unspecified source".into());
            changes.push(CorrelatedChange {
                category: "DEPLOYMENT".into(),
                event_type: "deployment.attempt".into(),
                occurred_at,
                minutes_from_incident: minutes_delta(incident_started_at, occurred_at),
                impact_related: related,
                resource_type: Some("SERVICE".into()),
                resource_id: Some(service_id),
                summary: format!(
                    "Deployment of {service_name} to {environment_name}: {status} ({source})"
                ),
            });
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    async fn load_releases(
        &self,
        organization_id: Uuid,
        project_id: Uuid,
        incident_started_at: DateTime<Utc>,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
        impacted: &HashSet<ResourceKey>,
        changes: &mut Vec<CorrelatedChange>,
    ) -> Result<(), sqlx::Error> {
        let rows = sqlx::query(
            "SELECT r.id,r.version,r.name,r.status,COALESCE(r.released_at,r.updated_at,r.created_at) AS occurred_at,ARRAY_REMOVE(ARRAY_AGG(rc.service_id),NULL) AS service_ids FROM releases r LEFT JOIN release_components rc ON rc.release_id=r.id WHERE r.organization_id=$1 AND r.project_id=$2 AND COALESCE(r.released_at,r.updated_at,r.created_at) BETWEEN $3 AND $4 GROUP BY r.id ORDER BY occurred_at",
        )
        .bind(organization_id)
        .bind(project_id)
        .bind(from)
        .bind(to)
        .fetch_all(&self.pool)
        .await?;
        for row in rows {
            let service_ids: Vec<Uuid> = row.try_get("service_ids").unwrap_or_default();
            let related = service_ids
                .iter()
                .any(|id| impacted.contains(&ResourceKey::new("SERVICE", *id)));
            let occurred_at: DateTime<Utc> = row.get("occurred_at");
            let name: String = row.get("name");
            let version: String = row.get("version");
            let status: String = row.get("status");
            changes.push(CorrelatedChange {
                category: "RELEASE".into(),
                event_type: "release.change".into(),
                occurred_at,
                minutes_from_incident: minutes_delta(incident_started_at, occurred_at),
                impact_related: related,
                resource_type: None,
                resource_id: None,
                summary: format!("Release {name} ({version}): {status}"),
            });
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    async fn load_commands(
        &self,
        organization_id: Uuid,
        project_id: Uuid,
        incident_started_at: DateTime<Utc>,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
        impacted: &HashSet<ResourceKey>,
        changes: &mut Vec<CorrelatedChange>,
    ) -> Result<(), sqlx::Error> {
        let rows = sqlx::query(
            "SELECT c.id,c.server_id,c.command_type,c.status,c.created_at,s.hostname FROM commands c JOIN servers s ON s.id=c.server_id WHERE s.organization_id=$1 AND s.project_id=$2 AND c.created_at BETWEEN $3 AND $4 ORDER BY c.created_at",
        )
        .bind(organization_id)
        .bind(project_id)
        .bind(from)
        .bind(to)
        .fetch_all(&self.pool)
        .await?;
        for row in rows {
            let server_id: Uuid = row.get("server_id");
            let occurred_at: DateTime<Utc> = row.get("created_at");
            let command_type: serde_json::Value = row.get("command_type");
            let kind = command_type
                .get("kind")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("unknown");
            let hostname: String = row.get("hostname");
            let status: String = row.get("status");
            changes.push(CorrelatedChange {
                category: "SERVER_COMMAND".into(),
                event_type: kind.into(),
                occurred_at,
                minutes_from_incident: minutes_delta(incident_started_at, occurred_at),
                impact_related: impacted.contains(&ResourceKey::new("SERVER", server_id)),
                resource_type: Some("SERVER".into()),
                resource_id: Some(server_id),
                summary: format!("{kind} on {hostname}: {status}"),
            });
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    async fn load_project_events(
        &self,
        organization_id: Uuid,
        project_id: Uuid,
        incident_started_at: DateTime<Utc>,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
        impacted: &HashSet<ResourceKey>,
        changes: &mut Vec<CorrelatedChange>,
    ) -> Result<(), sqlx::Error> {
        let rows = sqlx::query(
            "SELECT event_type,data,occurred_at FROM domain_events WHERE organization_id=$1 AND resource_id=$2 AND occurred_at BETWEEN $3 AND $4 AND (event_type LIKE 'service.%' OR event_type LIKE 'environment.%' OR event_type LIKE 'repository.%' OR event_type LIKE 'site.%' OR event_type LIKE 'domain.%' OR event_type LIKE 'dependency.%') AND event_type <> 'site.check.completed' ORDER BY occurred_at",
        )
        .bind(organization_id)
        .bind(project_id)
        .bind(from)
        .bind(to)
        .fetch_all(&self.pool)
        .await?;
        for row in rows {
            let event_type: String = row.get("event_type");
            let data: serde_json::Value = row.get("data");
            let occurred_at: DateTime<Utc> = row.get("occurred_at");
            let related_resource = related_resource_from_event(&data, impacted);
            changes.push(CorrelatedChange {
                category: "PROJECT_CHANGE".into(),
                event_type: event_type.clone(),
                occurred_at,
                minutes_from_incident: minutes_delta(incident_started_at, occurred_at),
                impact_related: related_resource.is_some(),
                resource_type: related_resource
                    .as_ref()
                    .map(|resource| resource.resource_type.clone()),
                resource_id: related_resource
                    .as_ref()
                    .map(|resource| resource.resource_id),
                summary: summarize_project_event(&event_type, &data),
            });
        }
        Ok(())
    }
}

pub fn router() -> Router<AppState> {
    Router::new().route(
        "/projects/:project_id/incidents/:incident_id/correlation",
        get(get_incident_correlation),
    )
}

async fn get_incident_correlation(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((project_id, incident_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<CorrelationView>, ApiError> {
    let identity = web_identity(&state, &headers).await?;
    Ok(Json(
        state
            .change_correlation
            .incident_view(identity.organization_id, project_id, incident_id)
            .await
            .map_err(map_correlation)?,
    ))
}

fn minutes_delta(incident: DateTime<Utc>, event: DateTime<Utc>) -> i64 {
    event.signed_duration_since(incident).num_minutes()
}

fn related_resource_from_event(
    data: &serde_json::Value,
    impacted: &HashSet<ResourceKey>,
) -> Option<ResourceKey> {
    for (key, resource_type) in [
        ("service_id", "SERVICE"),
        ("site_id", "SITE"),
        ("domain_id", "DOMAIN"),
        ("server_id", "SERVER"),
        ("environment_id", "ENVIRONMENT"),
        ("repository_id", "REPOSITORY"),
    ] {
        if let Some(id) = json_uuid(data.get(key)) {
            let resource = ResourceKey::new(resource_type, id);
            if impacted.contains(&resource) {
                return Some(resource);
            }
        }
    }
    if let (Some(resource_type), Some(resource_id)) = (
        data.get("source_type").and_then(serde_json::Value::as_str),
        json_uuid(data.get("source_id")),
    ) {
        let resource = ResourceKey::new(resource_type.to_uppercase(), resource_id);
        if impacted.contains(&resource) {
            return Some(resource);
        }
    }
    if let (Some(resource_type), Some(resource_id)) = (
        data.get("target_type").and_then(serde_json::Value::as_str),
        json_uuid(data.get("target_id")),
    ) {
        let resource = ResourceKey::new(resource_type.to_uppercase(), resource_id);
        if impacted.contains(&resource) {
            return Some(resource);
        }
    }
    None
}

fn json_uuid(value: Option<&serde_json::Value>) -> Option<Uuid> {
    value
        .and_then(serde_json::Value::as_str)
        .and_then(|value| Uuid::parse_str(value).ok())
}

fn summarize_project_event(event_type: &str, data: &serde_json::Value) -> String {
    let name = data
        .get("name")
        .or_else(|| data.get("hostname"))
        .or_else(|| data.get("version"))
        .and_then(serde_json::Value::as_str);
    match name {
        Some(name) => format!("{event_type}: {name}"),
        None => event_type.to_string(),
    }
}

fn map_correlation(error: ChangeCorrelationError) -> ApiError {
    match error {
        ChangeCorrelationError::NotFound => api_error(
            StatusCode::NOT_FOUND,
            "INCIDENT_NOT_FOUND",
            "incident not found",
        ),
        other => {
            tracing::error!(error=%other, "change correlation storage error");
            api_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "INTERNAL_ERROR",
                "internal error",
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn time_delta_keeps_before_after_direction() {
        let incident = DateTime::parse_from_rfc3339("2026-01-01T12:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        assert_eq!(
            minutes_delta(incident, incident - Duration::minutes(15)),
            -15
        );
        assert_eq!(
            minutes_delta(incident, incident + Duration::minutes(20)),
            20
        );
    }

    #[test]
    fn event_relation_uses_known_resource_ids() {
        let id = Uuid::new_v4();
        let impacted = HashSet::from([ResourceKey::new("SERVICE", id)]);
        let data = serde_json::json!({"service_id": id});
        assert!(related_resource_from_event(&data, &impacted).is_some());
        assert!(related_resource_from_event(&serde_json::json!({}), &impacted).is_none());
    }
}
