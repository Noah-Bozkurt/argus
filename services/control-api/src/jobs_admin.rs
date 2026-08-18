use crate::{ApiError, AppState, api_error, web_identity};
use axum::{
    Json, Router,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    routing::{get, post},
};
use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::Value;
use sqlx::{PgPool, Row};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct JobsAdminStore {
    pool: PgPool,
}

#[derive(Debug, Clone, Serialize)]
pub struct BackgroundJobView {
    pub id: Uuid,
    pub project_id: Option<Uuid>,
    pub project_name: Option<String>,
    pub job_kind: String,
    pub resource_key: String,
    pub payload: Value,
    pub status: String,
    pub run_at: DateTime<Utc>,
    pub attempts: i32,
    pub max_attempts: i32,
    pub lease_owner: Option<String>,
    pub lease_expires_at: Option<DateTime<Utc>>,
    pub last_error_code: Option<String>,
    pub last_error_message: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct JobScheduleView {
    pub id: Uuid,
    pub project_id: Option<Uuid>,
    pub project_name: Option<String>,
    pub job_kind: String,
    pub resource_key: String,
    pub interval_seconds: i32,
    pub max_attempts: i32,
    pub enabled: bool,
    pub next_run_at: DateTime<Utc>,
    pub last_enqueued_at: Option<DateTime<Utc>>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
pub struct JobsAdminView {
    pub queued_count: i64,
    pub running_count: i64,
    pub dead_count: i64,
    pub jobs: Vec<BackgroundJobView>,
    pub schedules: Vec<JobScheduleView>,
}

#[derive(Debug, thiserror::Error)]
pub enum JobsAdminError {
    #[error("job not found")]
    NotFound,
    #[error("job is not dead")]
    NotDead,
    #[error(transparent)]
    Sql(#[from] sqlx::Error),
}

impl JobsAdminStore {
    pub async fn connect(database_url: &str) -> Result<Self, sqlx::Error> {
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(10)
            .connect(database_url)
            .await?;
        Ok(Self { pool })
    }

    pub async fn view(&self, organization_id: Uuid) -> Result<JobsAdminView, JobsAdminError> {
        let counts = sqlx::query(
            "SELECT COUNT(*) FILTER (WHERE status='QUEUED') AS queued_count,COUNT(*) FILTER (WHERE status='RUNNING') AS running_count,COUNT(*) FILTER (WHERE status='DEAD') AS dead_count FROM background_jobs WHERE organization_id=$1",
        )
        .bind(organization_id)
        .fetch_one(&self.pool)
        .await?;

        let jobs = sqlx::query(
            "SELECT j.id,j.project_id,p.name AS project_name,j.job_kind,j.resource_key,j.payload,j.status,j.run_at,j.attempts,j.max_attempts,j.lease_owner,j.lease_expires_at,j.last_error_code,j.last_error_message,j.created_at,j.updated_at,j.completed_at FROM background_jobs j LEFT JOIN projects p ON p.id=j.project_id AND p.organization_id=j.organization_id WHERE j.organization_id=$1 ORDER BY j.created_at DESC LIMIT 200",
        )
        .bind(organization_id)
        .fetch_all(&self.pool)
        .await?
        .into_iter()
        .map(job_from_row)
        .collect();

        let schedules = sqlx::query(
            "SELECT s.id,s.project_id,p.name AS project_name,s.job_kind,s.resource_key,s.interval_seconds,s.max_attempts,s.enabled,s.next_run_at,s.last_enqueued_at,s.updated_at FROM job_schedules s LEFT JOIN projects p ON p.id=s.project_id AND p.organization_id=s.organization_id WHERE s.organization_id=$1 ORDER BY s.job_kind,p.name NULLS FIRST,s.resource_key",
        )
        .bind(organization_id)
        .fetch_all(&self.pool)
        .await?
        .into_iter()
        .map(schedule_from_row)
        .collect();

        Ok(JobsAdminView {
            queued_count: counts.get("queued_count"),
            running_count: counts.get("running_count"),
            dead_count: counts.get("dead_count"),
            jobs,
            schedules,
        })
    }

    pub async fn retry_dead(
        &self,
        identity: crate::persistence::WebIdentity,
        job_id: Uuid,
    ) -> Result<(), JobsAdminError> {
        let mut tx = self.pool.begin().await?;
        let status: Option<String> = sqlx::query_scalar(
            "SELECT status FROM background_jobs WHERE id=$1 AND organization_id=$2 FOR UPDATE",
        )
        .bind(job_id)
        .bind(identity.organization_id)
        .fetch_optional(&mut *tx)
        .await?;
        let Some(status) = status else {
            return Err(JobsAdminError::NotFound);
        };
        if status != "DEAD" {
            return Err(JobsAdminError::NotDead);
        }
        sqlx::query(
            "UPDATE background_jobs SET status='QUEUED',run_at=NOW(),attempts=0,lease_owner=NULL,lease_expires_at=NULL,last_error_code=NULL,last_error_message=NULL,completed_at=NULL,updated_at=NOW() WHERE id=$1 AND organization_id=$2",
        )
        .bind(job_id)
        .bind(identity.organization_id)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            "INSERT INTO audit_events(id,organization_id,actor,resource,action,request_id,result,source,timestamp) VALUES($1,$2,$3,$4,'background_job.retried',$5,'SUCCEEDED','web',NOW())",
        )
        .bind(Uuid::new_v4())
        .bind(identity.organization_id)
        .bind(identity.user_id.to_string())
        .bind(job_id.to_string())
        .bind(Uuid::new_v4().to_string())
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(())
    }
}

fn job_from_row(row: sqlx::postgres::PgRow) -> BackgroundJobView {
    BackgroundJobView {
        id: row.get("id"),
        project_id: row.get("project_id"),
        project_name: row.get("project_name"),
        job_kind: row.get("job_kind"),
        resource_key: row.get("resource_key"),
        payload: row.get("payload"),
        status: row.get("status"),
        run_at: row.get("run_at"),
        attempts: row.get("attempts"),
        max_attempts: row.get("max_attempts"),
        lease_owner: row.get("lease_owner"),
        lease_expires_at: row.get("lease_expires_at"),
        last_error_code: row.get("last_error_code"),
        last_error_message: row.get("last_error_message"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
        completed_at: row.get("completed_at"),
    }
}

fn schedule_from_row(row: sqlx::postgres::PgRow) -> JobScheduleView {
    JobScheduleView {
        id: row.get("id"),
        project_id: row.get("project_id"),
        project_name: row.get("project_name"),
        job_kind: row.get("job_kind"),
        resource_key: row.get("resource_key"),
        interval_seconds: row.get("interval_seconds"),
        max_attempts: row.get("max_attempts"),
        enabled: row.get("enabled"),
        next_run_at: row.get("next_run_at"),
        last_enqueued_at: row.get("last_enqueued_at"),
        updated_at: row.get("updated_at"),
    }
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/background-jobs", get(get_jobs))
        .route("/background-jobs/:job_id/retry", post(retry_dead_job))
}

async fn get_jobs(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<JobsAdminView>, ApiError> {
    let identity = web_identity(&state, &headers).await?;
    Ok(Json(
        state
            .jobs_admin
            .view(identity.organization_id)
            .await
            .map_err(map_jobs)?,
    ))
}

async fn retry_dead_job(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(job_id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    let identity = web_identity(&state, &headers).await?;
    state
        .jobs_admin
        .retry_dead(identity, job_id)
        .await
        .map_err(map_jobs)?;
    Ok(StatusCode::NO_CONTENT)
}

fn map_jobs(error: JobsAdminError) -> ApiError {
    match error {
        JobsAdminError::NotFound => api_error(
            StatusCode::NOT_FOUND,
            "JOB_NOT_FOUND",
            "background job not found",
        ),
        JobsAdminError::NotDead => api_error(
            StatusCode::CONFLICT,
            "JOB_NOT_DEAD",
            "only dead jobs can be retried",
        ),
        other => {
            tracing::error!(error=%other, "jobs administration storage error");
            api_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "INTERNAL_ERROR",
                "internal error",
            )
        }
    }
}
