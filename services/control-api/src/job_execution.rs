use crate::{ApiError, AppState, api_error, bearer_token};
use axum::{
    Json, Router,
    extract::State,
    http::{HeaderMap, StatusCode},
    routing::post,
};
use protocol::{CommandRequest, CommandType, RiskLevel};
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
        "site_monitor.check" => execute_site_monitor_check(&state, request).await,
        "site_incident.evaluate" => execute_site_incident_evaluation(&state, request).await,
        "desired_state.reconcile" => execute_desired_state_reconciliation(&state, request).await,
        "domains.lifecycle_evaluate" => execute_domain_lifecycle_evaluation(&state, request).await,
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

async fn execute_site_monitor_check(
    state: &AppState,
    request: ExecuteJobRequest,
) -> Result<Json<ExecuteJobResponse>, ApiError> {
    let project_id = request.project_id.ok_or_else(|| {
        api_error(
            StatusCode::BAD_REQUEST,
            "INVALID_JOB_PAYLOAD",
            "site_monitor.check requires a project",
        )
    })?;
    let site_id = payload_uuid(&request.payload, "site_id")?;
    let actor_user_id = payload_uuid(&request.payload, "actor_user_id")?;
    let identity = crate::persistence::WebIdentity {
        user_id: actor_user_id,
        organization_id: request.organization_id,
    };
    let check = state
        .site_monitoring
        .run_check(identity, project_id, site_id)
        .await
        .map_err(|error| {
            tracing::error!(job_id=%request.job_id, project_id=%project_id, site_id=%site_id, error=%error, "scheduled site monitor check failed");
            api_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "JOB_EXECUTION_FAILED",
                "scheduled site monitor check failed",
            )
        })?;

    Ok(Json(ExecuteJobResponse {
        job_id: request.job_id,
        status: "SUCCEEDED",
        summary: format!("site check completed with {}", check.overall_status),
    }))
}

async fn execute_site_incident_evaluation(
    state: &AppState,
    request: ExecuteJobRequest,
) -> Result<Json<ExecuteJobResponse>, ApiError> {
    let project_id = request.project_id.ok_or_else(|| {
        api_error(
            StatusCode::BAD_REQUEST,
            "INVALID_JOB_PAYLOAD",
            "site_incident.evaluate requires a project",
        )
    })?;
    let site_id = payload_uuid(&request.payload, "site_id")?;
    let check_id = payload_uuid(&request.payload, "check_id")?;
    let result = state
        .incident_automation
        .evaluate(
            state,
            request.organization_id,
            project_id,
            site_id,
            check_id,
        )
        .await
        .map_err(|error| {
            tracing::error!(job_id=%request.job_id, project_id=%project_id, site_id=%site_id, check_id=%check_id, error=%error, "Site Incident automation evaluation failed");
            api_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "JOB_EXECUTION_FAILED",
                "Site Incident automation evaluation failed",
            )
        })?;

    Ok(Json(ExecuteJobResponse {
        job_id: request.job_id,
        status: "SUCCEEDED",
        summary: match result.incident_id {
            Some(incident_id) => format!(
                "{} after {} consecutive failures: incident {}",
                result.action, result.consecutive_failures, incident_id
            ),
            None => format!(
                "{} after {} consecutive failures",
                result.action, result.consecutive_failures
            ),
        },
    }))
}

async fn execute_desired_state_reconciliation(
    state: &AppState,
    request: ExecuteJobRequest,
) -> Result<Json<ExecuteJobResponse>, ApiError> {
    let project_id = request.project_id.ok_or_else(|| {
        api_error(
            StatusCode::BAD_REQUEST,
            "INVALID_JOB_PAYLOAD",
            "desired_state.reconcile requires a project",
        )
    })?;
    let server_id = payload_uuid(&request.payload, "server_id")?;
    let actor_user_id = payload_uuid(&request.payload, "actor_user_id")?;
    let identity = crate::persistence::WebIdentity {
        user_id: actor_user_id,
        organization_id: request.organization_id,
    };

    let server = state.storage.get_server(identity, server_id).await.map_err(|error| {
        tracing::error!(job_id=%request.job_id, server_id=%server_id, error=%error, "desired state reconciliation server lookup failed");
        api_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "JOB_EXECUTION_FAILED",
            "desired state reconciliation server lookup failed",
        )
    })?;
    if server.project_id != project_id {
        return Err(api_error(
            StatusCode::BAD_REQUEST,
            "INVALID_JOB_PAYLOAD",
            "desired state reconciliation server/project mismatch",
        ));
    }

    let assessment = state
        .desired_state
        .assess_reconciliation(request.organization_id, server_id)
        .await
        .map_err(|error| {
            tracing::error!(job_id=%request.job_id, server_id=%server_id, error=%error, "desired state assessment failed");
            api_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "JOB_EXECUTION_FAILED",
                "desired state assessment failed",
            )
        })?;

    if !assessment.needs_firewall_enable {
        return Ok(Json(ExecuteJobResponse {
            job_id: request.job_id,
            status: "SUCCEEDED",
            summary: assessment.action.into(),
        }));
    }

    if state
        .maintenance
        .active(request.organization_id, server_id)
        .await
        .map_err(|error| {
            tracing::error!(job_id=%request.job_id, server_id=%server_id, error=%error, "desired state maintenance lookup failed");
            api_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "JOB_EXECUTION_FAILED",
                "desired state maintenance lookup failed",
            )
        })?
        .is_none()
    {
        return Ok(Json(ExecuteJobResponse {
            job_id: request.job_id,
            status: "SUCCEEDED",
            summary: "FIREWALL_DRIFT_BLOCKED_MAINTENANCE".into(),
        }));
    }

    let command = state
        .storage
        .queue_command(
            identity,
            CommandRequest {
                server_id,
                command_type: CommandType::SecurityFirewallEnable,
                ttl_seconds: 300,
                idempotency_key: format!("desired-state:{}:{}", server_id, request.job_id),
                risk_level: RiskLevel::HIGH,
            },
        )
        .await;

    let summary = match command {
        Ok(command) => format!("FIREWALL_RECONCILIATION_QUEUED {}", command.id),
        Err(crate::persistence::StorageError::Conflict) => {
            "FIREWALL_RECONCILIATION_ALREADY_QUEUED".into()
        }
        Err(error) => {
            tracing::error!(job_id=%request.job_id, server_id=%server_id, error=%error, "desired state command queue failed");
            return Err(api_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "JOB_EXECUTION_FAILED",
                "desired state command queue failed",
            ));
        }
    };

    Ok(Json(ExecuteJobResponse {
        job_id: request.job_id,
        status: "SUCCEEDED",
        summary,
    }))
}

async fn execute_domain_lifecycle_evaluation(
    state: &AppState,
    request: ExecuteJobRequest,
) -> Result<Json<ExecuteJobResponse>, ApiError> {
    if request.project_id.is_some() || !request.payload.is_object() {
        return Err(api_error(
            StatusCode::BAD_REQUEST,
            "INVALID_JOB_PAYLOAD",
            "domains.lifecycle_evaluate must be organization scoped",
        ));
    }
    let result = state
        .domain_lifecycle
        .evaluate_organization(request.organization_id)
        .await
        .map_err(|error| {
            tracing::error!(job_id=%request.job_id, organization_id=%request.organization_id, error=%error, "domain lifecycle evaluation failed");
            api_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "JOB_EXECUTION_FAILED",
                "domain lifecycle evaluation failed",
            )
        })?;
    Ok(Json(ExecuteJobResponse {
        job_id: request.job_id,
        status: "SUCCEEDED",
        summary: format!(
            "evaluated {} domains; {} changed",
            result.evaluated_domains, result.changed_domains
        ),
    }))
}

fn payload_uuid(payload: &Value, key: &str) -> Result<Uuid, ApiError> {
    payload
        .get(key)
        .and_then(Value::as_str)
        .and_then(|value| value.parse().ok())
        .ok_or_else(|| {
            api_error(
                StatusCode::BAD_REQUEST,
                "INVALID_JOB_PAYLOAD",
                "background job contains an invalid UUID field",
            )
        })
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
