use crate::{ApiError, AppState, api_error, web_identity};
use axum::{
    Json, Router,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    routing::{get, put},
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct IncidentAutomationStore {
    pool: PgPool,
}

#[derive(Debug, Clone, Serialize)]
pub struct SiteIncidentPolicyView {
    pub site_id: Uuid,
    pub site_name: String,
    pub has_monitor_config: bool,
    pub enabled: bool,
    pub failure_threshold: i32,
    pub severity: String,
    pub active_incident_id: Option<Uuid>,
    pub active_incident_status: Option<String>,
    pub updated_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize)]
pub struct SaveSiteIncidentPolicyRequest {
    pub enabled: bool,
    pub failure_threshold: i32,
    pub severity: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct IncidentAutomationResult {
    pub action: String,
    pub consecutive_failures: i32,
    pub incident_id: Option<Uuid>,
}

#[derive(Debug, thiserror::Error)]
pub enum IncidentAutomationError {
    #[error("site, monitor, or check not found")]
    NotFound,
    #[error("invalid incident automation policy")]
    Invalid,
    #[error(transparent)]
    Sql(#[from] sqlx::Error),
    #[error(transparent)]
    Incident(#[from] crate::incidents::IncidentError),
}

impl IncidentAutomationStore {
    pub async fn connect(database_url: &str) -> Result<Self, sqlx::Error> {
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(10)
            .connect(database_url)
            .await?;
        Ok(Self { pool })
    }

    pub async fn list(
        &self,
        organization_id: Uuid,
        project_id: Uuid,
    ) -> Result<Vec<SiteIncidentPolicyView>, IncidentAutomationError> {
        self.authorize_project(organization_id, project_id).await?;
        let rows = sqlx::query(
            "SELECT s.id AS site_id,s.name AS site_name,(m.id IS NOT NULL) AS has_monitor_config,COALESCE(p.enabled,FALSE) AS enabled,COALESCE(p.failure_threshold,3) AS failure_threshold,COALESCE(p.severity,'MAJOR') AS severity,p.active_incident_id,i.status AS active_incident_status,p.updated_at FROM sites s LEFT JOIN site_monitor_configs m ON m.site_id=s.id AND m.organization_id=s.organization_id AND m.project_id=s.project_id LEFT JOIN site_incident_policies p ON p.site_id=s.id AND p.organization_id=s.organization_id AND p.project_id=s.project_id LEFT JOIN incidents i ON i.id=p.active_incident_id AND i.organization_id=s.organization_id AND i.project_id=s.project_id WHERE s.organization_id=$1 AND s.project_id=$2 AND s.status='ACTIVE' ORDER BY s.name,s.id",
        )
        .bind(organization_id)
        .bind(project_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(|row| SiteIncidentPolicyView {
                site_id: row.get("site_id"),
                site_name: row.get("site_name"),
                has_monitor_config: row.get("has_monitor_config"),
                enabled: row.get("enabled"),
                failure_threshold: row.get("failure_threshold"),
                severity: row.get("severity"),
                active_incident_id: row.get("active_incident_id"),
                active_incident_status: row.get("active_incident_status"),
                updated_at: row.get("updated_at"),
            })
            .collect())
    }

    pub async fn save(
        &self,
        identity: crate::persistence::WebIdentity,
        project_id: Uuid,
        site_id: Uuid,
        request: SaveSiteIncidentPolicyRequest,
    ) -> Result<SiteIncidentPolicyView, IncidentAutomationError> {
        self.authorize_project(identity.organization_id, project_id)
            .await?;
        if !(2..=10).contains(&request.failure_threshold) {
            return Err(IncidentAutomationError::Invalid);
        }
        let severity = normalize_severity(&request.severity)?;
        let site_state = sqlx::query(
            "SELECT s.id,(m.id IS NOT NULL) AS has_monitor FROM sites s LEFT JOIN site_monitor_configs m ON m.site_id=s.id AND m.organization_id=s.organization_id AND m.project_id=s.project_id WHERE s.id=$1 AND s.organization_id=$2 AND s.project_id=$3 AND s.status='ACTIVE'",
        )
        .bind(site_id)
        .bind(identity.organization_id)
        .bind(project_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(IncidentAutomationError::NotFound)?;
        let has_monitor: bool = site_state.get("has_monitor");
        if request.enabled && !has_monitor {
            return Err(IncidentAutomationError::NotFound);
        }

        let mut tx = self.pool.begin().await?;
        sqlx::query(
            "INSERT INTO site_incident_policies(site_id,organization_id,project_id,enabled,failure_threshold,severity,configured_by,created_at,updated_at) VALUES($1,$2,$3,$4,$5,$6,$7,NOW(),NOW()) ON CONFLICT(site_id) DO UPDATE SET enabled=EXCLUDED.enabled,failure_threshold=EXCLUDED.failure_threshold,severity=EXCLUDED.severity,configured_by=EXCLUDED.configured_by,updated_at=NOW()",
        )
        .bind(site_id)
        .bind(identity.organization_id)
        .bind(project_id)
        .bind(request.enabled)
        .bind(request.failure_threshold)
        .bind(&severity)
        .bind(identity.user_id)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            "INSERT INTO audit_events(id,organization_id,actor,resource,action,request_id,result,source,timestamp) VALUES($1,$2,$3,$4,'site.incident_policy.updated',$5,'SUCCEEDED','web',NOW())",
        )
        .bind(Uuid::new_v4())
        .bind(identity.organization_id)
        .bind(identity.user_id.to_string())
        .bind(project_id.to_string())
        .bind(Uuid::new_v4().to_string())
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            "INSERT INTO domain_events(id,organization_id,event_type,resource_id,data,occurred_at) VALUES($1,$2,'site.incident_policy.updated',$3,$4,NOW())",
        )
        .bind(Uuid::new_v4())
        .bind(identity.organization_id)
        .bind(project_id)
        .bind(serde_json::json!({
            "site_id":site_id,
            "enabled":request.enabled,
            "failure_threshold":request.failure_threshold,
            "severity":severity
        }))
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;

        self.list(identity.organization_id, project_id)
            .await?
            .into_iter()
            .find(|policy| policy.site_id == site_id)
            .ok_or(IncidentAutomationError::NotFound)
    }

    pub async fn evaluate(
        &self,
        state: &AppState,
        organization_id: Uuid,
        project_id: Uuid,
        site_id: Uuid,
        check_id: Uuid,
    ) -> Result<IncidentAutomationResult, IncidentAutomationError> {
        self.authorize_project(organization_id, project_id).await?;
        let mut tx = self.pool.begin().await?;
        let policy_row = sqlx::query(
            "SELECT p.enabled,p.failure_threshold,p.severity,p.configured_by,p.active_incident_id,s.name AS site_name FROM site_incident_policies p JOIN sites s ON s.id=p.site_id AND s.organization_id=p.organization_id AND s.project_id=p.project_id WHERE p.site_id=$1 AND p.organization_id=$2 AND p.project_id=$3 FOR UPDATE OF p",
        )
        .bind(site_id)
        .bind(organization_id)
        .bind(project_id)
        .fetch_optional(&mut *tx)
        .await?;
        let Some(policy_row) = policy_row else {
            tx.rollback().await?;
            return Ok(no_action("DISABLED", 0));
        };
        let enabled: bool = policy_row.get("enabled");
        if !enabled {
            tx.rollback().await?;
            return Ok(no_action("DISABLED", 0));
        }
        let failure_threshold: i32 = policy_row.get("failure_threshold");
        let severity: String = policy_row.get("severity");
        let configured_by: Uuid = policy_row.get("configured_by");
        let site_name: String = policy_row.get("site_name");
        let mut active_incident_id: Option<Uuid> = policy_row.get("active_incident_id");

        let latest = sqlx::query(
            "SELECT id,overall_status,checked_at FROM site_monitor_checks WHERE organization_id=$1 AND project_id=$2 AND site_id=$3 ORDER BY checked_at DESC,id DESC LIMIT 1",
        )
        .bind(organization_id)
        .bind(project_id)
        .bind(site_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(IncidentAutomationError::NotFound)?;
        let latest_id: Uuid = latest.get("id");
        if latest_id != check_id {
            tx.rollback().await?;
            return Ok(no_action("STALE_CHECK", 0));
        }
        let latest_status: String = latest.get("overall_status");
        if !is_failure(&latest_status) {
            tx.rollback().await?;
            return Ok(no_action("HEALTHY_OR_DEGRADED", 0));
        }

        let mut after: Option<DateTime<Utc>> = None;
        if let Some(incident_id) = active_incident_id {
            let incident = sqlx::query(
                "SELECT status,resolved_at FROM incidents WHERE id=$1 AND organization_id=$2 AND project_id=$3",
            )
            .bind(incident_id)
            .bind(organization_id)
            .bind(project_id)
            .fetch_optional(&mut *tx)
            .await?;
            match incident {
                Some(row) => {
                    let status: String = row.get("status");
                    if status != "RESOLVED" {
                        tx.rollback().await?;
                        return Ok(IncidentAutomationResult {
                            action: "ACTIVE_INCIDENT_EXISTS".into(),
                            consecutive_failures: 0,
                            incident_id: Some(incident_id),
                        });
                    }
                    after = row.get("resolved_at");
                }
                None => {}
            }
            sqlx::query(
                "UPDATE site_incident_policies SET active_incident_id=NULL,updated_at=NOW() WHERE site_id=$1 AND organization_id=$2 AND project_id=$3",
            )
            .bind(site_id)
            .bind(organization_id)
            .bind(project_id)
            .execute(&mut *tx)
            .await?;
            active_incident_id = None;
        }

        let checks = if let Some(after) = after {
            sqlx::query(
                "SELECT overall_status FROM site_monitor_checks WHERE organization_id=$1 AND project_id=$2 AND site_id=$3 AND checked_at>$4 ORDER BY checked_at DESC,id DESC LIMIT $5",
            )
            .bind(organization_id)
            .bind(project_id)
            .bind(site_id)
            .bind(after)
            .bind(failure_threshold)
            .fetch_all(&mut *tx)
            .await?
        } else {
            sqlx::query(
                "SELECT overall_status FROM site_monitor_checks WHERE organization_id=$1 AND project_id=$2 AND site_id=$3 ORDER BY checked_at DESC,id DESC LIMIT $4",
            )
            .bind(organization_id)
            .bind(project_id)
            .bind(site_id)
            .bind(failure_threshold)
            .fetch_all(&mut *tx)
            .await?
        };
        let consecutive_failures = checks
            .iter()
            .take_while(|row| is_failure(row.get::<String, _>("overall_status").as_str()))
            .count() as i32;
        if consecutive_failures < failure_threshold {
            tx.commit().await?;
            return Ok(no_action("THRESHOLD_NOT_MET", consecutive_failures));
        }

        let identity = crate::persistence::WebIdentity {
            user_id: configured_by,
            organization_id,
        };
        let incident = state
            .incidents
            .create(
                state,
                identity,
                project_id,
                crate::incidents::CreateIncidentRequest {
                    title: format!("Site unavailable: {site_name}"),
                    summary: format!(
                        "Automatically created after {failure_threshold} consecutive DOWN/ERROR Site Monitoring checks. Investigate and resolve this Incident manually."
                    ),
                    severity,
                    source_type: "SITE".into(),
                    source_id: site_id,
                },
            )
            .await?;
        let incident_id = incident.incident.id;
        sqlx::query(
            "UPDATE site_incident_policies SET active_incident_id=$4,updated_at=NOW() WHERE site_id=$1 AND organization_id=$2 AND project_id=$3",
        )
        .bind(site_id)
        .bind(organization_id)
        .bind(project_id)
        .bind(incident_id)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(IncidentAutomationResult {
            action: "INCIDENT_CREATED".into(),
            consecutive_failures,
            incident_id: Some(incident_id),
        })
    }

    async fn authorize_project(
        &self,
        organization_id: Uuid,
        project_id: Uuid,
    ) -> Result<(), IncidentAutomationError> {
        let exists: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM projects WHERE id=$1 AND organization_id=$2)",
        )
        .bind(project_id)
        .bind(organization_id)
        .fetch_one(&self.pool)
        .await?;
        if exists {
            Ok(())
        } else {
            Err(IncidentAutomationError::NotFound)
        }
    }
}

fn normalize_severity(value: &str) -> Result<String, IncidentAutomationError> {
    let severity = value.trim().to_uppercase();
    if matches!(severity.as_str(), "MINOR" | "MAJOR" | "CRITICAL") {
        Ok(severity)
    } else {
        Err(IncidentAutomationError::Invalid)
    }
}

fn is_failure(status: &str) -> bool {
    matches!(status, "DOWN" | "ERROR")
}

fn no_action(action: &str, consecutive_failures: i32) -> IncidentAutomationResult {
    IncidentAutomationResult {
        action: action.into(),
        consecutive_failures,
        incident_id: None,
    }
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/projects/:project_id/site-incident-policies",
            get(list_policies),
        )
        .route(
            "/projects/:project_id/sites/:site_id/incident-policy",
            put(save_policy),
        )
}

async fn list_policies(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(project_id): Path<Uuid>,
) -> Result<Json<Vec<SiteIncidentPolicyView>>, ApiError> {
    let identity = web_identity(&state, &headers).await?;
    Ok(Json(
        state
            .incident_automation
            .list(identity.organization_id, project_id)
            .await
            .map_err(map_automation)?,
    ))
}

async fn save_policy(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((project_id, site_id)): Path<(Uuid, Uuid)>,
    Json(request): Json<SaveSiteIncidentPolicyRequest>,
) -> Result<Json<SiteIncidentPolicyView>, ApiError> {
    let identity = web_identity(&state, &headers).await?;
    Ok(Json(
        state
            .incident_automation
            .save(identity, project_id, site_id, request)
            .await
            .map_err(map_automation)?,
    ))
}

fn map_automation(error: IncidentAutomationError) -> ApiError {
    match error {
        IncidentAutomationError::NotFound => api_error(
            StatusCode::NOT_FOUND,
            "SITE_MONITOR_NOT_FOUND",
            "site or monitor configuration not found",
        ),
        IncidentAutomationError::Invalid => api_error(
            StatusCode::BAD_REQUEST,
            "INVALID_REQUEST",
            "invalid Site Incident automation policy",
        ),
        other => {
            tracing::error!(error=%other, "site incident automation error");
            api_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "INTERNAL_ERROR",
                "internal error",
            )
        }
    }
}
