use crate::{ApiError, AppState, api_error, bearer_token};
use axum::{
    Json, Router,
    extract::State,
    http::{HeaderMap, StatusCode},
    routing::post,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

#[derive(Debug, Deserialize)]
pub struct ExecuteJobRequest {
    pub job_id: Uuid,
    pub organization_id: Uuid,
    pub project_id: Option<Uuid>,
    pub kind: String,
    #[serde(default)]
    pub payload: Value,
}

#[derive(Debug, Serialize)]
pub struct ExecuteJobResponse {
    pub job_id: Uuid,
    pub status: &'static str,
    pub summary: String,
}

pub fn router() -> Router<AppState> {
    Router::new().route("/internal/jobs/execute", post(execute_job))
}

async fn execute_job(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<ExecuteJobRequest>,
) -> Result<Json<ExecuteJobResponse>, ApiError> {
    authorize_worker(&state, &headers)?;
    match request.kind.as_str() {
        "notifications.materialize" => execute_notification_materialization(&state, request).await,
        _ => Err(api_error(
            StatusCode::BAD_REQUEST,
            "JOB_KIND_UNSUPPORTED",
            "unsupported background job kind",
        )),
    }
}

async fn execute_notification_materialization(
    state: &AppState,
    request: ExecuteJobRequest,
) -> Result<Json<ExecuteJobResponse>, ApiError> {
    if request.project_id.is_some() || !request.payload.is_object() {
        return Err(api_error(
            StatusCode::BAD_REQUEST,
            "INVALID_JOB_PAYLOAD",
            "notifications.materialize must be organization scoped",
        ));
    }

    // V1 reuses the same idempotent materializer as the manual refresh path.
    // Uuid::nil() is a reserved system-actor sentinel; it is never authenticated as a user.
    let system_identity = crate::persistence::WebIdentity {
        user_id: Uuid::nil(),
        organization_id: request.organization_id,
    };
    let result = state
        .notifications
        .sync(system_identity)
        .await
        .map_err(|error| {
            tracing::error!(job_id=%request.job_id, error=%error, "notification materialization job failed");
            api_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "JOB_EXECUTION_FAILED",
                "notification materialization failed",
            )
        })?;

    Ok(Json(ExecuteJobResponse {
        job_id: request.job_id,
        status: "SUCCEEDED",
        summary: format!(
            "scanned {} events and created {} notifications",
            result.scanned_events, result.created_notifications
        ),
    }))
}

fn authorize_worker(state: &AppState, headers: &HeaderMap) -> Result<(), ApiError> {
    let expected = state.worker_api_token.as_deref().ok_or_else(|| {
        api_error(
            StatusCode::SERVICE_UNAVAILABLE,
            "WORKER_API_DISABLED",
            "worker API token is not configured",
        )
    })?;
    let supplied = bearer_token(headers)?;
    if supplied == expected.as_str() {
        Ok(())
    } else {
        Err(api_error(
            StatusCode::UNAUTHORIZED,
            "PERMISSION_DENIED",
            "invalid worker credential",
        ))
    }
}
