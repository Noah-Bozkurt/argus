use anyhow::{Context, Result, bail};
use chrono::{DateTime, Utc};
use reqwest::Client;
use serde::Serialize;
use serde_json::Value;
use sqlx::{PgPool, Row};
use std::time::Duration;
use tracing::{error, info, warn};
use uuid::Uuid;

const CLAIM_BATCH: usize = 20;

#[derive(Debug)]
struct Config {
    database_url: String,
    control_api_url: String,
    worker_token: String,
    poll_seconds: u64,
    lease_seconds: i32,
}

impl Config {
    fn from_env() -> Result<Self> {
        let database_url = std::env::var("DATABASE_URL").context("DATABASE_URL is required")?;
        if !database_url.starts_with("postgres://") && !database_url.starts_with("postgresql://") {
            bail!("DATABASE_URL must be PostgreSQL");
        }
        let control_api_url = std::env::var("ARGUS_CONTROL_API_URL")
            .unwrap_or_else(|_| "http://127.0.0.1:8080".into());
        let worker_token =
            std::env::var("ARGUS_WORKER_TOKEN").context("ARGUS_WORKER_TOKEN is required")?;
        if worker_token.len() < 32 {
            bail!("ARGUS_WORKER_TOKEN must be at least 32 characters");
        }
        let poll_seconds = std::env::var("ARGUS_WORKER_POLL_SECONDS")
            .ok()
            .and_then(|value| value.parse().ok())
            .unwrap_or(2_u64)
            .clamp(1, 60);
        let lease_seconds = std::env::var("ARGUS_WORKER_LEASE_SECONDS")
            .ok()
            .and_then(|value| value.parse().ok())
            .unwrap_or(60_i32)
            .clamp(15, 900);
        Ok(Self {
            database_url,
            control_api_url,
            worker_token,
            poll_seconds,
            lease_seconds,
        })
    }
}

#[derive(Debug)]
struct Job {
    id: Uuid,
    organization_id: Uuid,
    project_id: Option<Uuid>,
    job_kind: String,
    payload: Value,
    attempts: i32,
    max_attempts: i32,
}

#[derive(Debug, Serialize)]
struct ExecuteJobRequest<'a> {
    job_id: Uuid,
    organization_id: Uuid,
    project_id: Option<Uuid>,
    kind: &'a str,
    payload: &'a Value,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();
    let config = Config::from_env()?;
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(5)
        .connect(&config.database_url)
        .await
        .context("connect worker database")?;
    let client = Client::builder()
        .timeout(Duration::from_secs(60))
        .build()
        .context("build worker HTTP client")?;
    let worker_id = format!("worker-{}", Uuid::new_v4());
    info!(worker_id, "starting Argus worker");

    if let Err(err) = run_cycle(&pool, &client, &config, &worker_id).await {
        error!(error = %err, "initial worker cycle failed");
    }

    loop {
        tokio::select! {
            result = tokio::signal::ctrl_c() => {
                result.context("listen for shutdown signal")?;
                info!(worker_id, "worker shutdown requested");
                break;
            }
            _ = tokio::time::sleep(Duration::from_secs(config.poll_seconds)) => {
                if let Err(err) = run_cycle(&pool, &client, &config, &worker_id).await {
                    error!(error = %err, "worker cycle failed");
                }
            }
        }
    }
    Ok(())
}

async fn run_cycle(pool: &PgPool, client: &Client, config: &Config, worker_id: &str) -> Result<()> {
    let enqueued = enqueue_due_schedules(pool).await?;
    if enqueued > 0 {
        info!(enqueued, "materialized due job schedules");
    }
    expire_exhausted_jobs(pool).await?;

    for _ in 0..CLAIM_BATCH {
        let Some(job) = claim_job(pool, worker_id, config.lease_seconds).await? else {
            break;
        };
        let started = Utc::now();
        match execute_job(client, config, &job).await {
            Ok(()) => {
                mark_succeeded(pool, job.id).await?;
                info!(job_id=%job.id, kind=%job.job_kind, elapsed_ms=(Utc::now()-started).num_milliseconds(), "job succeeded");
            }
            Err(err) => {
                let message = truncate(&format!("{err:#}"), 1000);
                mark_failed_or_retry(pool, &job, "EXECUTION_FAILED", &message).await?;
                warn!(job_id=%job.id, kind=%job.job_kind, attempts=job.attempts, error=%message, "job execution failed");
            }
        }
    }
    Ok(())
}

async fn enqueue_due_schedules(pool: &PgPool) -> Result<u64> {
    let mut tx = pool.begin().await?;
    let rows = sqlx::query(
        "SELECT id,organization_id,project_id,job_kind,resource_key,payload,interval_seconds,max_attempts,next_run_at FROM job_schedules WHERE enabled=TRUE AND next_run_at<=NOW() ORDER BY next_run_at,id LIMIT 50 FOR UPDATE SKIP LOCKED",
    )
    .fetch_all(&mut *tx)
    .await?;
    let mut created = 0_u64;
    for row in rows {
        let schedule_id: Uuid = row.get("id");
        let organization_id: Uuid = row.get("organization_id");
        let project_id: Option<Uuid> = row.get("project_id");
        let job_kind: String = row.get("job_kind");
        let resource_key: String = row.get("resource_key");
        let payload: Value = row.get("payload");
        let interval_seconds: i32 = row.get("interval_seconds");
        let max_attempts: i32 = row.get("max_attempts");
        let scheduled_for: DateTime<Utc> = row.get("next_run_at");
        let dedupe_key = format!("schedule:{schedule_id}:{}", scheduled_for.to_rfc3339());
        let result = sqlx::query(
            "INSERT INTO background_jobs(id,organization_id,project_id,schedule_id,job_kind,resource_key,payload,dedupe_key,status,run_at,attempts,max_attempts,created_at,updated_at) VALUES($1,$2,$3,$4,$5,$6,$7,$8,'QUEUED',NOW(),0,$9,NOW(),NOW()) ON CONFLICT DO NOTHING",
        )
        .bind(Uuid::new_v4())
        .bind(organization_id)
        .bind(project_id)
        .bind(schedule_id)
        .bind(&job_kind)
        .bind(&resource_key)
        .bind(payload)
        .bind(dedupe_key)
        .bind(max_attempts)
        .execute(&mut *tx)
        .await?;
        created += result.rows_affected();
        sqlx::query(
            "UPDATE job_schedules SET last_enqueued_at=NOW(),next_run_at=GREATEST($2 + (interval_seconds * INTERVAL '1 second'),NOW() + (interval_seconds * INTERVAL '1 second')),updated_at=NOW() WHERE id=$1",
        )
        .bind(schedule_id)
        .bind(scheduled_for)
        .execute(&mut *tx)
        .await?;
        let _ = interval_seconds;
    }
    tx.commit().await?;
    Ok(created)
}

async fn expire_exhausted_jobs(pool: &PgPool) -> Result<()> {
    sqlx::query(
        "UPDATE background_jobs SET status='DEAD',lease_owner=NULL,lease_expires_at=NULL,completed_at=COALESCE(completed_at,NOW()),updated_at=NOW(),last_error_code=COALESCE(last_error_code,'MAX_ATTEMPTS_EXCEEDED') WHERE status IN ('QUEUED','RUNNING') AND attempts>=max_attempts AND (status='QUEUED' OR lease_expires_at IS NULL OR lease_expires_at<NOW())",
    )
    .execute(pool)
    .await?;
    Ok(())
}

async fn claim_job(pool: &PgPool, worker_id: &str, lease_seconds: i32) -> Result<Option<Job>> {
    let row = sqlx::query(
        "WITH candidate AS (SELECT id FROM background_jobs WHERE attempts<max_attempts AND ((status='QUEUED' AND run_at<=NOW()) OR (status='RUNNING' AND lease_expires_at<NOW())) ORDER BY run_at,created_at LIMIT 1 FOR UPDATE SKIP LOCKED) UPDATE background_jobs j SET status='RUNNING',attempts=j.attempts+1,lease_owner=$1,lease_expires_at=NOW()+($2 * INTERVAL '1 second'),updated_at=NOW() FROM candidate c WHERE j.id=c.id RETURNING j.id,j.organization_id,j.project_id,j.job_kind,j.payload,j.attempts,j.max_attempts",
    )
    .bind(worker_id)
    .bind(lease_seconds)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|row| Job {
        id: row.get("id"),
        organization_id: row.get("organization_id"),
        project_id: row.get("project_id"),
        job_kind: row.get("job_kind"),
        payload: row.get("payload"),
        attempts: row.get("attempts"),
        max_attempts: row.get("max_attempts"),
    }))
}

async fn execute_job(client: &Client, config: &Config, job: &Job) -> Result<()> {
    let endpoint = format!(
        "{}/internal/jobs/execute",
        config.control_api_url.trim_end_matches('/')
    );
    let response = client
        .post(endpoint)
        .bearer_auth(&config.worker_token)
        .json(&ExecuteJobRequest {
            job_id: job.id,
            organization_id: job.organization_id,
            project_id: job.project_id,
            kind: &job.job_kind,
            payload: &job.payload,
        })
        .send()
        .await
        .context("call Control API job executor")?;
    if response.status().is_success() {
        return Ok(());
    }
    let status = response.status();
    let body = response.text().await.unwrap_or_default();
    bail!("Control API returned {status}: {}", truncate(&body, 500));
}

async fn mark_succeeded(pool: &PgPool, job_id: Uuid) -> Result<()> {
    sqlx::query(
        "UPDATE background_jobs SET status='SUCCEEDED',lease_owner=NULL,lease_expires_at=NULL,last_error_code=NULL,last_error_message=NULL,completed_at=NOW(),updated_at=NOW() WHERE id=$1",
    )
    .bind(job_id)
    .execute(pool)
    .await?;
    Ok(())
}

async fn mark_failed_or_retry(pool: &PgPool, job: &Job, code: &str, message: &str) -> Result<()> {
    if job.attempts >= job.max_attempts {
        sqlx::query(
            "UPDATE background_jobs SET status='DEAD',lease_owner=NULL,lease_expires_at=NULL,last_error_code=$2,last_error_message=$3,completed_at=NOW(),updated_at=NOW() WHERE id=$1",
        )
        .bind(job.id)
        .bind(code)
        .bind(message)
        .execute(pool)
        .await?;
        return Ok(());
    }
    let exponent = (job.attempts.saturating_sub(1) as u32).min(6);
    let delay_seconds = (5_i64 * (1_i64 << exponent)).min(300);
    sqlx::query(
        "UPDATE background_jobs SET status='QUEUED',run_at=NOW()+($2 * INTERVAL '1 second'),lease_owner=NULL,lease_expires_at=NULL,last_error_code=$3,last_error_message=$4,updated_at=NOW() WHERE id=$1",
    )
    .bind(job.id)
    .bind(delay_seconds)
    .bind(code)
    .bind(message)
    .execute(pool)
    .await?;
    Ok(())
}

fn truncate(value: &str, max_chars: usize) -> String {
    value.chars().take(max_chars).collect()
}
