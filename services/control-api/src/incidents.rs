use crate::{ApiError, AppState, api_error, web_identity};
use axum::{
    Json, Router,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    routing::{get, post},
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct IncidentStore {
    pool: PgPool,
}

#[derive(Debug, Clone, Serialize)]
pub struct IncidentSummary {
    pub id: Uuid,
    pub project_id: Uuid,
    pub title: String,
    pub summary: String,
    pub severity: String,
    pub status: String,
    pub source_type: String,
    pub source_id: Uuid,
    pub source_name: String,
    pub affected_count: i64,
    pub created_by: Uuid,
    pub started_at: DateTime<Utc>,
    pub resolved_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
pub struct IncidentAffectedResource {
    pub id: Uuid,
    pub resource_type: String,
    pub resource_id: Uuid,
    pub resource_name: String,
    pub distance: i32,
    pub impact_path: serde_json::Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
pub struct IncidentTimelineEvent {
    pub id: Uuid,
    pub event_type: String,
    pub message: String,
    pub data: serde_json::Value,
    pub created_by: Uuid,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
pub struct IncidentDetail {
    pub incident: IncidentSummary,
    pub affected: Vec<IncidentAffectedResource>,
    pub timeline: Vec<IncidentTimelineEvent>,
}

#[derive(Debug, Deserialize)]
pub struct CreateIncidentRequest {
    pub title: String,
    #[serde(default)]
    pub summary: String,
    pub severity: String,
    pub source_type: String,
    pub source_id: Uuid,
}

#[derive(Debug, Deserialize)]
pub struct UpdateIncidentStatusRequest {
    pub status: String,
}

#[derive(Debug, Deserialize)]
pub struct AddIncidentNoteRequest {
    pub message: String,
}

#[derive(Debug, thiserror::Error)]
pub enum IncidentError {
    #[error("incident or resource not found")]
    NotFound,
    #[error("invalid incident request")]
    Invalid,
    #[error("incident state conflict")]
    Conflict,
    #[error(transparent)]
    Sql(#[from] sqlx::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

impl IncidentStore {
    pub async fn connect(database_url: &str) -> Result<Self, sqlx::Error> {
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(10)
            .connect(database_url)
            .await?;
        Ok(Self { pool })
    }

    async fn authorize_project(
        &self,
        organization_id: Uuid,
        project_id: Uuid,
    ) -> Result<(), IncidentError> {
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
            Err(IncidentError::NotFound)
        }
    }

    pub async fn list(
        &self,
        organization_id: Uuid,
        project_id: Uuid,
    ) -> Result<Vec<IncidentSummary>, IncidentError> {
        self.authorize_project(organization_id, project_id).await?;
        let rows = sqlx::query(
            "SELECT i.id,i.project_id,i.title,i.summary,i.severity,i.status,i.source_type,i.source_id,i.source_name,i.created_by,i.started_at,i.resolved_at,i.created_at,i.updated_at,COUNT(a.id) AS affected_count FROM incidents i LEFT JOIN incident_affected_resources a ON a.incident_id=i.id WHERE i.organization_id=$1 AND i.project_id=$2 GROUP BY i.id ORDER BY CASE i.status WHEN 'RESOLVED' THEN 1 ELSE 0 END,i.started_at DESC LIMIT 200",
        )
        .bind(organization_id)
        .bind(project_id)
        .fetch_all(&self.pool)
        .await?;
        rows.into_iter().map(summary_from_row).collect()
    }

    pub async fn detail(
        &self,
        organization_id: Uuid,
        project_id: Uuid,
        incident_id: Uuid,
    ) -> Result<IncidentDetail, IncidentError> {
        self.authorize_project(organization_id, project_id).await?;
        let incident_row = sqlx::query(
            "SELECT i.id,i.project_id,i.title,i.summary,i.severity,i.status,i.source_type,i.source_id,i.source_name,i.created_by,i.started_at,i.resolved_at,i.created_at,i.updated_at,COUNT(a.id) AS affected_count FROM incidents i LEFT JOIN incident_affected_resources a ON a.incident_id=i.id WHERE i.id=$1 AND i.organization_id=$2 AND i.project_id=$3 GROUP BY i.id",
        )
        .bind(incident_id)
        .bind(organization_id)
        .bind(project_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(IncidentError::NotFound)?;
        let affected_rows = sqlx::query(
            "SELECT id,resource_type,resource_id,resource_name,distance,impact_path,created_at FROM incident_affected_resources WHERE incident_id=$1 AND organization_id=$2 AND project_id=$3 ORDER BY distance,resource_type,resource_name",
        )
        .bind(incident_id)
        .bind(organization_id)
        .bind(project_id)
        .fetch_all(&self.pool)
        .await?;
        let timeline_rows = sqlx::query(
            "SELECT id,event_type,message,data,created_by,created_at FROM incident_timeline WHERE incident_id=$1 AND organization_id=$2 AND project_id=$3 ORDER BY created_at,id",
        )
        .bind(incident_id)
        .bind(organization_id)
        .bind(project_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(IncidentDetail {
            incident: summary_from_row(incident_row)?,
            affected: affected_rows
                .into_iter()
                .map(affected_from_row)
                .collect::<Result<Vec<_>, _>>()?,
            timeline: timeline_rows
                .into_iter()
                .map(timeline_from_row)
                .collect::<Result<Vec<_>, _>>()?,
        })
    }

    pub async fn create(
        &self,
        state: &AppState,
        identity: crate::persistence::WebIdentity,
        project_id: Uuid,
        request: CreateIncidentRequest,
    ) -> Result<IncidentDetail, IncidentError> {
        self.authorize_project(identity.organization_id, project_id)
            .await?;
        let title = required_text(&request.title, 1, 240)?;
        let summary = optional_text(&request.summary, 12000)?;
        let severity = normalize_severity(&request.severity)?;
        let source_type = request.source_type.trim().to_uppercase();
        let impact = state
            .dependency_graph
            .impact(
                identity.organization_id,
                project_id,
                &source_type,
                request.source_id,
            )
            .await
            .map_err(|_| IncidentError::NotFound)?;

        let incident_id = Uuid::new_v4();
        let mut tx = self.pool.begin().await?;
        sqlx::query(
            "INSERT INTO incidents(id,organization_id,project_id,title,summary,severity,status,source_type,source_id,source_name,created_by,started_at,created_at,updated_at) VALUES($1,$2,$3,$4,$5,$6,'INVESTIGATING',$7,$8,$9,$10,NOW(),NOW(),NOW())",
        )
        .bind(incident_id)
        .bind(identity.organization_id)
        .bind(project_id)
        .bind(&title)
        .bind(&summary)
        .bind(&severity)
        .bind(&impact.root.resource_type)
        .bind(impact.root.resource_id)
        .bind(&impact.root.name)
        .bind(identity.user_id)
        .execute(&mut *tx)
        .await?;
        for affected in &impact.affected {
            sqlx::query(
                "INSERT INTO incident_affected_resources(id,organization_id,project_id,incident_id,resource_type,resource_id,resource_name,distance,impact_path,created_at) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,NOW())",
            )
            .bind(Uuid::new_v4())
            .bind(identity.organization_id)
            .bind(project_id)
            .bind(incident_id)
            .bind(&affected.resource.resource_type)
            .bind(affected.resource.resource_id)
            .bind(&affected.resource.name)
            .bind(affected.distance as i32)
            .bind(serde_json::to_value(&affected.path)?)
            .execute(&mut *tx)
            .await?;
        }
        insert_timeline(
            &mut tx,
            identity,
            project_id,
            incident_id,
            "CREATED",
            &format!("Incident created: {title}"),
            serde_json::json!({
                "severity":severity,
                "source_type":impact.root.resource_type,
                "source_id":impact.root.resource_id,
                "affected_count":impact.affected_count
            }),
        )
        .await?;
        touch_project(&mut tx, identity.organization_id, project_id).await?;
        audit_event(
            &mut tx,
            identity,
            project_id,
            "incident.created",
            serde_json::json!({
                "incident_id":incident_id,
                "severity":severity,
                "source_type":impact.root.resource_type,
                "source_id":impact.root.resource_id,
                "affected_count":impact.affected_count
            }),
        )
        .await?;
        tx.commit().await?;
        self.detail(identity.organization_id, project_id, incident_id)
            .await
    }

    pub async fn update_status(
        &self,
        identity: crate::persistence::WebIdentity,
        project_id: Uuid,
        incident_id: Uuid,
        request: UpdateIncidentStatusRequest,
    ) -> Result<IncidentDetail, IncidentError> {
        self.authorize_project(identity.organization_id, project_id)
            .await?;
        let current: String = sqlx::query_scalar(
            "SELECT status FROM incidents WHERE id=$1 AND organization_id=$2 AND project_id=$3",
        )
        .bind(incident_id)
        .bind(identity.organization_id)
        .bind(project_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(IncidentError::NotFound)?;
        let next = request.status.trim().to_uppercase();
        if !incident_transition_allowed(&current, &next) {
            return Err(IncidentError::Conflict);
        }
        let mut tx = self.pool.begin().await?;
        sqlx::query(
            "UPDATE incidents SET status=$1,resolved_at=CASE WHEN $1='RESOLVED' THEN NOW() ELSE NULL END,updated_at=NOW() WHERE id=$2 AND organization_id=$3 AND project_id=$4",
        )
        .bind(&next)
        .bind(incident_id)
        .bind(identity.organization_id)
        .bind(project_id)
        .execute(&mut *tx)
        .await?;
        insert_timeline(
            &mut tx,
            identity,
            project_id,
            incident_id,
            "STATUS_CHANGED",
            &format!("Status changed from {current} to {next}"),
            serde_json::json!({"from":current,"to":next}),
        )
        .await?;
        touch_project(&mut tx, identity.organization_id, project_id).await?;
        audit_event(
            &mut tx,
            identity,
            project_id,
            "incident.status_changed",
            serde_json::json!({"incident_id":incident_id,"from":current,"to":next}),
        )
        .await?;
        tx.commit().await?;
        self.detail(identity.organization_id, project_id, incident_id)
            .await
    }

    pub async fn add_note(
        &self,
        identity: crate::persistence::WebIdentity,
        project_id: Uuid,
        incident_id: Uuid,
        request: AddIncidentNoteRequest,
    ) -> Result<IncidentDetail, IncidentError> {
        self.authorize_project(identity.organization_id, project_id)
            .await?;
        let exists: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM incidents WHERE id=$1 AND organization_id=$2 AND project_id=$3)",
        )
        .bind(incident_id)
        .bind(identity.organization_id)
        .bind(project_id)
        .fetch_one(&self.pool)
        .await?;
        if !exists {
            return Err(IncidentError::NotFound);
        }
        let message = required_text(&request.message, 1, 12000)?;
        let mut tx = self.pool.begin().await?;
        insert_timeline(
            &mut tx,
            identity,
            project_id,
            incident_id,
            "NOTE",
            &message,
            serde_json::json!({}),
        )
        .await?;
        touch_project(&mut tx, identity.organization_id, project_id).await?;
        audit_event(
            &mut tx,
            identity,
            project_id,
            "incident.note_added",
            serde_json::json!({"incident_id":incident_id}),
        )
        .await?;
        tx.commit().await?;
        self.detail(identity.organization_id, project_id, incident_id)
            .await
    }
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/projects/:project_id/incidents",
            get(list_incidents).post(create_incident),
        )
        .route(
            "/projects/:project_id/incidents/:incident_id",
            get(get_incident),
        )
        .route(
            "/projects/:project_id/incidents/:incident_id/status",
            axum::routing::put(update_incident_status),
        )
        .route(
            "/projects/:project_id/incidents/:incident_id/notes",
            post(add_incident_note),
        )
}

async fn list_incidents(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(project_id): Path<Uuid>,
) -> Result<Json<Vec<IncidentSummary>>, ApiError> {
    let identity = web_identity(&state, &headers).await?;
    Ok(Json(
        state
            .incidents
            .list(identity.organization_id, project_id)
            .await
            .map_err(map_incident)?,
    ))
}

async fn get_incident(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((project_id, incident_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<IncidentDetail>, ApiError> {
    let identity = web_identity(&state, &headers).await?;
    Ok(Json(
        state
            .incidents
            .detail(identity.organization_id, project_id, incident_id)
            .await
            .map_err(map_incident)?,
    ))
}

async fn create_incident(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(project_id): Path<Uuid>,
    Json(request): Json<CreateIncidentRequest>,
) -> Result<(StatusCode, Json<IncidentDetail>), ApiError> {
    let identity = web_identity(&state, &headers).await?;
    let incident = state
        .incidents
        .create(&state, identity, project_id, request)
        .await
        .map_err(map_incident)?;
    Ok((StatusCode::CREATED, Json(incident)))
}

async fn update_incident_status(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((project_id, incident_id)): Path<(Uuid, Uuid)>,
    Json(request): Json<UpdateIncidentStatusRequest>,
) -> Result<Json<IncidentDetail>, ApiError> {
    let identity = web_identity(&state, &headers).await?;
    Ok(Json(
        state
            .incidents
            .update_status(identity, project_id, incident_id, request)
            .await
            .map_err(map_incident)?,
    ))
}

async fn add_incident_note(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((project_id, incident_id)): Path<(Uuid, Uuid)>,
    Json(request): Json<AddIncidentNoteRequest>,
) -> Result<Json<IncidentDetail>, ApiError> {
    let identity = web_identity(&state, &headers).await?;
    Ok(Json(
        state
            .incidents
            .add_note(identity, project_id, incident_id, request)
            .await
            .map_err(map_incident)?,
    ))
}

fn summary_from_row(row: sqlx::postgres::PgRow) -> Result<IncidentSummary, IncidentError> {
    Ok(IncidentSummary {
        id: row.get("id"),
        project_id: row.get("project_id"),
        title: row.get("title"),
        summary: row.get("summary"),
        severity: row.get("severity"),
        status: row.get("status"),
        source_type: row.get("source_type"),
        source_id: row.get("source_id"),
        source_name: row.get("source_name"),
        affected_count: row.get("affected_count"),
        created_by: row.get("created_by"),
        started_at: row.get("started_at"),
        resolved_at: row.get("resolved_at"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}

fn affected_from_row(
    row: sqlx::postgres::PgRow,
) -> Result<IncidentAffectedResource, IncidentError> {
    Ok(IncidentAffectedResource {
        id: row.get("id"),
        resource_type: row.get("resource_type"),
        resource_id: row.get("resource_id"),
        resource_name: row.get("resource_name"),
        distance: row.get("distance"),
        impact_path: row.get("impact_path"),
        created_at: row.get("created_at"),
    })
}

fn timeline_from_row(row: sqlx::postgres::PgRow) -> Result<IncidentTimelineEvent, IncidentError> {
    Ok(IncidentTimelineEvent {
        id: row.get("id"),
        event_type: row.get("event_type"),
        message: row.get("message"),
        data: row.get("data"),
        created_by: row.get("created_by"),
        created_at: row.get("created_at"),
    })
}

fn normalize_severity(value: &str) -> Result<String, IncidentError> {
    let value = value.trim().to_uppercase();
    if matches!(value.as_str(), "MINOR" | "MAJOR" | "CRITICAL") {
        Ok(value)
    } else {
        Err(IncidentError::Invalid)
    }
}

fn incident_transition_allowed(current: &str, next: &str) -> bool {
    matches!(
        (current, next),
        ("INVESTIGATING", "IDENTIFIED")
            | ("INVESTIGATING", "MONITORING")
            | ("IDENTIFIED", "INVESTIGATING")
            | ("IDENTIFIED", "MONITORING")
            | ("MONITORING", "INVESTIGATING")
            | ("MONITORING", "RESOLVED")
    )
}

fn required_text(value: &str, min: usize, max: usize) -> Result<String, IncidentError> {
    let value = value.trim();
    if value.len() < min || value.len() > max {
        Err(IncidentError::Invalid)
    } else {
        Ok(value.to_string())
    }
}

fn optional_text(value: &str, max: usize) -> Result<String, IncidentError> {
    let value = value.trim();
    if value.len() > max {
        Err(IncidentError::Invalid)
    } else {
        Ok(value.to_string())
    }
}

async fn insert_timeline(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    identity: crate::persistence::WebIdentity,
    project_id: Uuid,
    incident_id: Uuid,
    event_type: &str,
    message: &str,
    data: serde_json::Value,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO incident_timeline(id,organization_id,project_id,incident_id,event_type,message,data,created_by,created_at) VALUES($1,$2,$3,$4,$5,$6,$7,$8,NOW())",
    )
    .bind(Uuid::new_v4())
    .bind(identity.organization_id)
    .bind(project_id)
    .bind(incident_id)
    .bind(event_type)
    .bind(message)
    .bind(data)
    .bind(identity.user_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

async fn touch_project(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    organization_id: Uuid,
    project_id: Uuid,
) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE projects SET updated_at=NOW() WHERE id=$1 AND organization_id=$2")
        .bind(project_id)
        .bind(organization_id)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

async fn audit_event(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    identity: crate::persistence::WebIdentity,
    project_id: Uuid,
    event_type: &str,
    data: serde_json::Value,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO audit_events(id,organization_id,actor,resource,action,request_id,result,source,timestamp) VALUES($1,$2,$3,$4,$5,$6,'SUCCEEDED','web',NOW())",
    )
    .bind(Uuid::new_v4())
    .bind(identity.organization_id)
    .bind(identity.user_id.to_string())
    .bind(project_id.to_string())
    .bind(event_type)
    .bind(Uuid::new_v4().to_string())
    .execute(&mut **tx)
    .await?;
    sqlx::query(
        "INSERT INTO domain_events(id,organization_id,event_type,resource_id,data,occurred_at) VALUES($1,$2,$3,$4,$5,NOW())",
    )
    .bind(Uuid::new_v4())
    .bind(identity.organization_id)
    .bind(event_type)
    .bind(project_id)
    .bind(data)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

fn map_incident(error: IncidentError) -> ApiError {
    match error {
        IncidentError::NotFound => api_error(
            StatusCode::NOT_FOUND,
            "INCIDENT_NOT_FOUND",
            "incident or source resource not found",
        ),
        IncidentError::Invalid => api_error(
            StatusCode::BAD_REQUEST,
            "INVALID_REQUEST",
            "invalid incident request",
        ),
        IncidentError::Conflict => api_error(
            StatusCode::CONFLICT,
            "OPERATION_CONFLICT",
            "incident status transition is not allowed",
        ),
        other => {
            tracing::error!(error=%other, "incident storage error");
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
    fn severity_is_explicit() {
        assert_eq!(normalize_severity("critical").unwrap(), "CRITICAL");
        assert!(normalize_severity("HIGH").is_err());
    }

    #[test]
    fn incident_lifecycle_supports_regression_but_resolved_is_terminal() {
        assert!(incident_transition_allowed("INVESTIGATING", "IDENTIFIED"));
        assert!(incident_transition_allowed("IDENTIFIED", "MONITORING"));
        assert!(incident_transition_allowed("MONITORING", "INVESTIGATING"));
        assert!(incident_transition_allowed("MONITORING", "RESOLVED"));
        assert!(!incident_transition_allowed("RESOLVED", "INVESTIGATING"));
        assert!(!incident_transition_allowed("INVESTIGATING", "RESOLVED"));
    }
}
