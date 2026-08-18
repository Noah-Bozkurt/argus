use crate::{ApiError, AppState, api_error, web_identity};
use axum::{
    Json, Router,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    routing::get,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};
use uuid::Uuid;

const JOB_KIND: &str = "site_monitor.check";

#[derive(Debug, Clone)]
pub struct MonitorSchedulingStore {
    pool: PgPool,
}

#[derive(Debug, Clone, Serialize)]
pub struct MonitorSchedule {
    pub site_id: Uuid,
    pub site_name: String,
    pub has_monitor_config: bool,
    pub schedule_id: Option<Uuid>,
    pub enabled: bool,
    pub interval_seconds: i32,
    pub next_run_at: Option<DateTime<Utc>>,
    pub last_enqueued_at: Option<DateTime<Utc>>,
    pub actor_user_id: Option<Uuid>,
}

#[derive(Debug, Deserialize)]
pub struct SaveMonitorScheduleRequest {
    pub enabled: bool,
    pub interval_seconds: i32,
}

#[derive(Debug, thiserror::Error)]
pub enum MonitorSchedulingError {
    #[error("site or monitor configuration not found")]
    NotFound,
    #[error("invalid monitor schedule")]
    Invalid,
    #[error(transparent)]
    Sql(#[from] sqlx::Error),
}

impl MonitorSchedulingStore {
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
    ) -> Result<Vec<MonitorSchedule>, MonitorSchedulingError> {
        self.authorize_project(organization_id, project_id).await?;
        let rows = sqlx::query(
            "SELECT s.id AS site_id,s.name AS site_name,(c.id IS NOT NULL) AS has_monitor_config,j.id AS schedule_id,COALESCE(j.enabled,FALSE) AS enabled,COALESCE(j.interval_seconds,300) AS interval_seconds,j.next_run_at,j.last_enqueued_at,j.payload->>'actor_user_id' AS actor_user_id FROM sites s LEFT JOIN site_monitor_configs c ON c.site_id=s.id AND c.organization_id=s.organization_id AND c.project_id=s.project_id LEFT JOIN job_schedules j ON j.organization_id=s.organization_id AND j.project_id=s.project_id AND j.job_kind=$3 AND j.resource_key=s.id::text WHERE s.organization_id=$1 AND s.project_id=$2 AND s.status='ACTIVE' ORDER BY s.name,s.id",
        )
        .bind(organization_id)
        .bind(project_id)
        .bind(JOB_KIND)
        .fetch_all(&self.pool)
        .await?;
        rows.into_iter()
            .map(|row| {
                let actor_text: Option<String> = row.get("actor_user_id");
                let actor_user_id = actor_text.and_then(|value| value.parse().ok());
                Ok(MonitorSchedule {
                    site_id: row.get("site_id"),
                    site_name: row.get("site_name"),
                    has_monitor_config: row.get("has_monitor_config"),
                    schedule_id: row.get("schedule_id"),
                    enabled: row.get("enabled"),
                    interval_seconds: row.get("interval_seconds"),
                    next_run_at: row.get("next_run_at"),
                    last_enqueued_at: row.get("last_enqueued_at"),
                    actor_user_id,
                })
            })
            .collect()
    }

    pub async fn save(
        &self,
        identity: crate::persistence::WebIdentity,
        project_id: Uuid,
        site_id: Uuid,
        request: SaveMonitorScheduleRequest,
    ) -> Result<MonitorSchedule, MonitorSchedulingError> {
        if !(60..=86_400).contains(&request.interval_seconds) {
            return Err(MonitorSchedulingError::Invalid);
        }
        self.authorize_project(identity.organization_id, project_id)
            .await?;
        let configured: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM sites s JOIN site_monitor_configs c ON c.site_id=s.id AND c.organization_id=s.organization_id AND c.project_id=s.project_id WHERE s.id=$1 AND s.organization_id=$2 AND s.project_id=$3 AND s.status='ACTIVE')",
        )
        .bind(site_id)
        .bind(identity.organization_id)
        .bind(project_id)
        .fetch_one(&self.pool)
        .await?;
        if !configured {
            return Err(MonitorSchedulingError::NotFound);
        }

        let payload = serde_json::json!({
            "site_id": site_id,
            "actor_user_id": identity.user_id,
        });
        let resource_key = site_id.to_string();
        let mut tx = self.pool.begin().await?;
        sqlx::query(
            "INSERT INTO job_schedules(id,organization_id,project_id,job_kind,resource_key,payload,interval_seconds,max_attempts,enabled,next_run_at,created_at,updated_at) VALUES($1,$2,$3,$4,$5,$6,$7,5,$8,NOW(),NOW(),NOW()) ON CONFLICT(organization_id,job_kind,resource_key) DO UPDATE SET project_id=EXCLUDED.project_id,payload=EXCLUDED.payload,interval_seconds=EXCLUDED.interval_seconds,enabled=EXCLUDED.enabled,next_run_at=CASE WHEN EXCLUDED.enabled AND NOT job_schedules.enabled THEN NOW() ELSE LEAST(job_schedules.next_run_at,NOW() + (EXCLUDED.interval_seconds * INTERVAL '1 second')) END,updated_at=NOW()",
        )
        .bind(Uuid::new_v4())
        .bind(identity.organization_id)
        .bind(project_id)
        .bind(JOB_KIND)
        .bind(&resource_key)
        .bind(payload)
        .bind(request.interval_seconds)
        .bind(request.enabled)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            "INSERT INTO audit_events(id,organization_id,actor,resource,action,request_id,result,source,timestamp) VALUES($1,$2,$3,$4,'site.monitor.schedule_updated',$5,'SUCCEEDED','web',NOW())",
        )
        .bind(Uuid::new_v4())
        .bind(identity.organization_id)
        .bind(identity.user_id.to_string())
        .bind(project_id.to_string())
        .bind(Uuid::new_v4().to_string())
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            "INSERT INTO domain_events(id,organization_id,event_type,resource_id,data,occurred_at) VALUES($1,$2,'site.monitor.schedule_updated',$3,$4,NOW())",
        )
        .bind(Uuid::new_v4())
        .bind(identity.organization_id)
        .bind(project_id)
        .bind(serde_json::json!({
            "site_id": site_id,
            "enabled": request.enabled,
            "interval_seconds": request.interval_seconds,
        }))
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;

        self.list(identity.organization_id, project_id)
            .await?
            .into_iter()
            .find(|schedule| schedule.site_id == site_id)
            .ok_or(MonitorSchedulingError::NotFound)
    }

    async fn authorize_project(
        &self,
        organization_id: Uuid,
        project_id: Uuid,
    ) -> Result<(), MonitorSchedulingError> {
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
            Err(MonitorSchedulingError::NotFound)
        }
    }
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/projects/:project_id/site-monitoring/schedules",
            get(list_schedules),
        )
        .route(
            "/projects/:project_id/sites/:site_id/monitor/schedule",
            axum::routing::put(save_schedule),
        )
}

async fn list_schedules(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(project_id): Path<Uuid>,
) -> Result<Json<Vec<MonitorSchedule>>, ApiError> {
    let identity = web_identity(&state, &headers).await?;
    Ok(Json(
        state
            .monitor_scheduling
            .list(identity.organization_id, project_id)
            .await
            .map_err(map_scheduling)?,
    ))
}

async fn save_schedule(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((project_id, site_id)): Path<(Uuid, Uuid)>,
    Json(request): Json<SaveMonitorScheduleRequest>,
) -> Result<Json<MonitorSchedule>, ApiError> {
    let identity = web_identity(&state, &headers).await?;
    Ok(Json(
        state
            .monitor_scheduling
            .save(identity, project_id, site_id, request)
            .await
            .map_err(map_scheduling)?,
    ))
}

fn map_scheduling(error: MonitorSchedulingError) -> ApiError {
    match error {
        MonitorSchedulingError::NotFound => api_error(
            StatusCode::NOT_FOUND,
            "SITE_MONITOR_NOT_FOUND",
            "site or monitor configuration not found",
        ),
        MonitorSchedulingError::Invalid => api_error(
            StatusCode::BAD_REQUEST,
            "INVALID_REQUEST",
            "monitor interval must be between 60 seconds and 24 hours",
        ),
        other => {
            tracing::error!(error=%other, "monitor scheduling storage error");
            api_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "INTERNAL_ERROR",
                "internal error",
            )
        }
    }
}
