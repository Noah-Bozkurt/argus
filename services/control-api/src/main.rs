use axum::{
    Json, Router,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use protocol::{
    CommandRequest, CommandResult, EnrollmentRequest, EnrollmentResponse, HeartbeatRequest,
    validate_protocol_version,
};
use serde::{Deserialize, Serialize};
use std::{net::SocketAddr, sync::Arc};
use tracing::info;
use uuid::Uuid;

mod change_correlation;
mod compose_stacks;
mod dependency_graph;
mod deployments_releases;
mod desired_state;
mod environments;
mod github_integration;
mod incident_automation;
mod incidents;
mod job_execution;
mod jobs_admin;
mod maintenance;
mod monitor_scheduling;
mod notifications;
mod persistence;
mod project_workspace;
mod readiness;
mod service_catalog;
mod site_monitoring;
mod sites_domains;
mod status_pages;
use change_correlation::ChangeCorrelationStore;
use compose_stacks::ComposeStackStore;
use dependency_graph::DependencyGraphStore;
use deployments_releases::DeploymentReleaseStore;
use desired_state::{DesiredState, DesiredStateError, DesiredStateStore};
use environments::EnvironmentStore;
use github_integration::{GitHubIntegrationStore, GitHubProvider};
use incident_automation::IncidentAutomationStore;
use incidents::IncidentStore;
use jobs_admin::JobsAdminStore;
use maintenance::MaintenanceStore;
use monitor_scheduling::MonitorSchedulingStore;
use notifications::NotificationStore;
use persistence::{Storage, StorageError, WebIdentity};
use project_workspace::ProjectWorkspaceStore;
use readiness::ReadinessStore;
use service_catalog::ServiceCatalogStore;
use site_monitoring::SiteMonitoringStore;
use sites_domains::SiteDomainStore;
use status_pages::StatusPageStore;

#[derive(Clone)]
struct AppState {
    storage: Storage,
    jobs_admin: JobsAdminStore,
    maintenance: MaintenanceStore,
    monitor_scheduling: MonitorSchedulingStore,
    notifications: NotificationStore,
    incident_automation: IncidentAutomationStore,
    incidents: IncidentStore,
    desired_state: DesiredStateStore,
    change_correlation: ChangeCorrelationStore,
    dependency_graph: DependencyGraphStore,
    deployments_releases: DeploymentReleaseStore,
    environments: EnvironmentStore,
    compose_stacks: ComposeStackStore,
    workspace: ProjectWorkspaceStore,
    readiness: ReadinessStore,
    github: GitHubIntegrationStore,
    service_catalog: ServiceCatalogStore,
    sites_domains: SiteDomainStore,
    status_pages: StatusPageStore,
    site_monitoring: SiteMonitoringStore,
    web_api_token: Arc<String>,
    worker_api_token: Option<Arc<String>>,
}
#[derive(Debug)]
struct Config {
    bind_addr: SocketAddr,
    database_url: String,
    web_api_token: String,
    worker_api_token: Option<String>,
}
impl Config {
    fn from_env() -> Result<Self, String> {
        let bind_addr = std::env::var("ARGUS_CONTROL_API_BIND")
            .unwrap_or_else(|_| "0.0.0.0:8080".into())
            .parse()
            .map_err(|e| format!("invalid bind address: {e}"))?;
        let database_url =
            std::env::var("DATABASE_URL").map_err(|_| "DATABASE_URL is required".to_string())?;
        if !database_url.starts_with("postgres://") && !database_url.starts_with("postgresql://") {
            return Err("DATABASE_URL must be PostgreSQL".into());
        }
        let web_api_token = std::env::var("ARGUS_WEB_API_TOKEN")
            .map_err(|_| "ARGUS_WEB_API_TOKEN is required".to_string())?;
        if web_api_token.len() < 32 {
            return Err("ARGUS_WEB_API_TOKEN must be at least 32 characters".into());
        }
        let worker_api_token = std::env::var("ARGUS_WORKER_TOKEN").ok();
        if worker_api_token
            .as_ref()
            .is_some_and(|token| token.len() < 32)
        {
            return Err("ARGUS_WORKER_TOKEN must be at least 32 characters when configured".into());
        }
        Ok(Self {
            bind_addr,
            database_url,
            web_api_token,
            worker_api_token,
        })
    }
}
#[derive(Debug, Serialize)]
struct ErrorResponse {
    code: String,
    message: String,
}
type ApiError = (StatusCode, Json<ErrorResponse>);
#[derive(Debug, Deserialize)]
struct CreateServerRequest {
    project_id: Uuid,
    environment_id: Uuid,
    hostname: String,
}
#[derive(Debug, Serialize)]
struct CreateServerResponse {
    server_id: Uuid,
}
#[derive(Debug, Deserialize)]
struct CreateEnrollmentTokenRequest {
    server_id: Uuid,
    ttl_seconds: i64,
}
#[derive(Debug, Serialize)]
struct CreateEnrollmentTokenResponse {
    token: String,
    expires_at: chrono::DateTime<chrono::Utc>,
}
#[derive(Debug, Deserialize)]
struct StartMaintenanceRequest {
    duration_minutes: i64,
    reason: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();
    let config = Config::from_env().map_err(anyhow::Error::msg)?;
    let storage = Storage::connect(&config.database_url).await?;
    let jobs_admin = JobsAdminStore::connect(&config.database_url).await?;
    let maintenance = MaintenanceStore::connect(&config.database_url).await?;
    let monitor_scheduling = MonitorSchedulingStore::connect(&config.database_url).await?;
    let notifications = NotificationStore::connect(&config.database_url).await?;
    let incident_automation = IncidentAutomationStore::connect(&config.database_url).await?;
    let incidents = IncidentStore::connect(&config.database_url).await?;
    let desired_state = DesiredStateStore::connect(&config.database_url).await?;
    let change_correlation = ChangeCorrelationStore::connect(&config.database_url).await?;
    let dependency_graph = DependencyGraphStore::connect(&config.database_url).await?;
    let deployments_releases = DeploymentReleaseStore::connect(&config.database_url).await?;
    let environments = EnvironmentStore::connect(&config.database_url).await?;
    let compose_stacks = ComposeStackStore::connect(&config.database_url).await?;
    let workspace = ProjectWorkspaceStore::connect(&config.database_url).await?;
    let readiness = ReadinessStore::connect(&config.database_url).await?;
    let github =
        GitHubIntegrationStore::connect(&config.database_url, GitHubProvider::from_env()?).await?;
    let service_catalog = ServiceCatalogStore::connect(&config.database_url).await?;
    let sites_domains = SiteDomainStore::connect(&config.database_url).await?;
    let status_pages = StatusPageStore::connect(&config.database_url).await?;
    let site_monitoring = SiteMonitoringStore::connect(&config.database_url).await?;
    let state = AppState {
        storage,
        jobs_admin,
        maintenance,
        monitor_scheduling,
        notifications,
        incident_automation,
        incidents,
        desired_state,
        change_correlation,
        dependency_graph,
        deployments_releases,
        environments,
        compose_stacks,
        workspace,
        readiness,
        github,
        service_catalog,
        sites_domains,
        status_pages,
        site_monitoring,
        web_api_token: Arc::new(config.web_api_token),
        worker_api_token: config.worker_api_token.map(Arc::new),
    };
    let app = Router::new()
        .route("/health", get(health))
        .route("/servers", get(list_servers).post(create_server))
        .route("/servers/:server_id", get(get_server))
        .route("/servers/:server_id/commands", get(command_history))
        .route("/servers/:server_id/maintenance", get(maintenance_history))
        .route(
            "/servers/:server_id/maintenance/start",
            post(start_maintenance),
        )
        .route("/servers/:server_id/maintenance/end", post(end_maintenance))
        .route(
            "/servers/:server_id/desired-state",
            get(get_desired_state).put(update_desired_state),
        )
        .route("/commands", post(queue_command))
        .route("/enrollment/tokens", post(create_enrollment_token))
        .route("/enrollment/complete", post(complete_enrollment))
        .route("/agent/identity", get(agent_identity))
        .route("/agent/heartbeat", post(heartbeat))
        .route("/agent/commands/next", post(next_command))
        .route("/agent/commands/result", post(command_result))
        .merge(project_workspace::router())
        .merge(github_integration::router())
        .merge(service_catalog::router())
        .merge(environments::router())
        .merge(compose_stacks::router())
        .merge(deployments_releases::router())
        .merge(sites_domains::router())
        .merge(site_monitoring::router())
        .merge(monitor_scheduling::router())
        .merge(dependency_graph::router())
        .merge(incident_automation::router())
        .merge(incidents::router())
        .merge(change_correlation::router())
        .merge(readiness::router())
        .merge(status_pages::router())
        .merge(notifications::router())
        .merge(job_execution::router())
        .merge(jobs_admin::router())
        .with_state(state);
    info!(bind_addr=%config.bind_addr, "starting persistent Argus control API");
    let listener = tokio::net::TcpListener::bind(config.bind_addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}
async fn health() -> &'static str {
    "ok"
}

async fn web_identity(state: &AppState, headers: &HeaderMap) -> Result<WebIdentity, ApiError> {
    let bearer = bearer_token(headers)?;
    if bearer != state.web_api_token.as_str() {
        return Err(api_error(
            StatusCode::UNAUTHORIZED,
            "PERMISSION_DENIED",
            "invalid web backend credential",
        ));
    }
    let identity = WebIdentity {
        user_id: uuid_header(headers, "x-argus-user-id")?,
        organization_id: uuid_header(headers, "x-argus-org-id")?,
    };
    state
        .storage
        .verify_web_identity(identity)
        .await
        .map_err(map_storage)?;
    Ok(identity)
}
async fn list_servers(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<persistence::ServerView>>, ApiError> {
    let identity = web_identity(&state, &headers).await?;
    Ok(Json(
        state
            .storage
            .list_servers(identity)
            .await
            .map_err(map_storage)?,
    ))
}
async fn get_server(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(server_id): Path<Uuid>,
) -> Result<Json<persistence::ServerView>, ApiError> {
    let identity = web_identity(&state, &headers).await?;
    Ok(Json(
        state
            .storage
            .get_server(identity, server_id)
            .await
            .map_err(map_storage)?,
    ))
}
async fn create_server(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<CreateServerRequest>,
) -> Result<Json<CreateServerResponse>, ApiError> {
    let identity = web_identity(&state, &headers).await?;
    let hostname = req.hostname.trim();
    if hostname.is_empty() || hostname.len() > 255 {
        return Err(api_error(
            StatusCode::BAD_REQUEST,
            "INVALID_REQUEST",
            "invalid hostname",
        ));
    }
    Ok(Json(CreateServerResponse {
        server_id: state
            .storage
            .create_server(identity, req.project_id, req.environment_id, hostname)
            .await
            .map_err(map_storage)?,
    }))
}
async fn create_enrollment_token(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<CreateEnrollmentTokenRequest>,
) -> Result<Json<CreateEnrollmentTokenResponse>, ApiError> {
    let identity = web_identity(&state, &headers).await?;
    let (token, expires_at) = state
        .storage
        .create_enrollment_token(identity, req.server_id, req.ttl_seconds)
        .await
        .map_err(map_storage)?;
    Ok(Json(CreateEnrollmentTokenResponse { token, expires_at }))
}
async fn complete_enrollment(
    State(state): State<AppState>,
    Json(req): Json<EnrollmentRequest>,
) -> Result<Json<EnrollmentResponse>, ApiError> {
    validate_protocol_version(&req.handshake.protocol_version).map_err(|_| {
        api_error(
            StatusCode::BAD_REQUEST,
            "CAPABILITY_UNAVAILABLE",
            "unsupported protocol version",
        )
    })?;
    Ok(Json(EnrollmentResponse {
        credential: state
            .storage
            .complete_enrollment(&req.token, &req.handshake)
            .await
            .map_err(map_storage)?,
    }))
}
async fn agent_identity(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<StatusCode, ApiError> {
    state
        .storage
        .authenticate_agent(bearer_token(&headers)?)
        .await
        .map_err(map_storage)?;
    Ok(StatusCode::NO_CONTENT)
}
async fn heartbeat(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<HeartbeatRequest>,
) -> Result<StatusCode, ApiError> {
    state
        .storage
        .heartbeat(bearer_token(&headers)?, &req)
        .await
        .map_err(map_storage)?;
    Ok(StatusCode::NO_CONTENT)
}

async fn start_maintenance(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(server_id): Path<Uuid>,
    Json(req): Json<StartMaintenanceRequest>,
) -> Result<Json<maintenance::MaintenanceWindow>, ApiError> {
    let identity = web_identity(&state, &headers).await?;
    let reason = req.reason.trim();
    if !(1..=1440).contains(&req.duration_minutes) || reason.is_empty() || reason.len() > 500 {
        return Err(api_error(
            StatusCode::BAD_REQUEST,
            "INVALID_REQUEST",
            "maintenance duration must be 1-1440 minutes and reason 1-500 characters",
        ));
    }
    let window = state
        .maintenance
        .start(
            identity.organization_id,
            identity.user_id,
            server_id,
            req.duration_minutes,
            reason,
        )
        .await
        .map_err(map_maintenance)?;
    Ok(Json(window))
}
async fn end_maintenance(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(server_id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    let identity = web_identity(&state, &headers).await?;
    state
        .maintenance
        .end_active(identity.organization_id, server_id)
        .await
        .map_err(map_maintenance)?;
    Ok(StatusCode::NO_CONTENT)
}
async fn maintenance_history(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(server_id): Path<Uuid>,
) -> Result<Json<Vec<maintenance::MaintenanceWindow>>, ApiError> {
    let identity = web_identity(&state, &headers).await?;
    Ok(Json(
        state
            .maintenance
            .history(identity.organization_id, server_id)
            .await
            .map_err(map_maintenance)?,
    ))
}

async fn get_desired_state(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(server_id): Path<Uuid>,
) -> Result<Json<desired_state::DesiredStateView>, ApiError> {
    let identity = web_identity(&state, &headers).await?;
    Ok(Json(
        state
            .desired_state
            .get(identity, server_id)
            .await
            .map_err(map_desired_state)?,
    ))
}

async fn update_desired_state(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(server_id): Path<Uuid>,
    Json(policy): Json<DesiredState>,
) -> Result<Json<desired_state::DesiredStateView>, ApiError> {
    let identity = web_identity(&state, &headers).await?;
    Ok(Json(
        state
            .desired_state
            .update(identity, server_id, policy)
            .await
            .map_err(map_desired_state)?,
    ))
}

async fn queue_command(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<CommandRequest>,
) -> Result<Json<protocol::Command>, ApiError> {
    let identity = web_identity(&state, &headers).await?;
    if req.command_type.requires_maintenance()
        && state
            .maintenance
            .active(identity.organization_id, req.server_id)
            .await
            .map_err(map_maintenance)?
            .is_none()
    {
        return Err(api_error(
            StatusCode::CONFLICT,
            "MAINTENANCE_REQUIRED",
            "this operation requires an active maintenance window",
        ));
    }
    Ok(Json(
        state
            .storage
            .queue_command(identity, req)
            .await
            .map_err(map_storage)?,
    ))
}
async fn next_command(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    match state
        .storage
        .claim_next_command(bearer_token(&headers)?)
        .await
        .map_err(map_storage)?
    {
        Some(command) => Ok(Json(command).into_response()),
        None => Ok(StatusCode::NO_CONTENT.into_response()),
    }
}
async fn command_result(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(result): Json<CommandResult>,
) -> Result<StatusCode, ApiError> {
    state
        .storage
        .submit_command_result(bearer_token(&headers)?, result)
        .await
        .map_err(map_storage)?;
    Ok(StatusCode::NO_CONTENT)
}
async fn command_history(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(server_id): Path<Uuid>,
) -> Result<Json<Vec<persistence::CommandHistoryItem>>, ApiError> {
    let identity = web_identity(&state, &headers).await?;
    Ok(Json(
        state
            .storage
            .command_history(identity, server_id)
            .await
            .map_err(map_storage)?,
    ))
}

fn bearer_token(headers: &HeaderMap) -> Result<&str, ApiError> {
    let value = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| {
            api_error(
                StatusCode::UNAUTHORIZED,
                "PERMISSION_DENIED",
                "missing authorization",
            )
        })?;
    value
        .strip_prefix("Bearer ")
        .filter(|v| !v.is_empty())
        .ok_or_else(|| {
            api_error(
                StatusCode::UNAUTHORIZED,
                "PERMISSION_DENIED",
                "invalid authorization",
            )
        })
}
fn uuid_header(headers: &HeaderMap, name: &str) -> Result<Uuid, ApiError> {
    headers
        .get(name)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse().ok())
        .ok_or_else(|| {
            api_error(
                StatusCode::UNAUTHORIZED,
                "PERMISSION_DENIED",
                &format!("missing or invalid {name}"),
            )
        })
}
fn map_storage(error: StorageError) -> ApiError {
    match error {
        StorageError::NotFound => api_error(
            StatusCode::NOT_FOUND,
            "SERVICE_NOT_FOUND",
            "resource not found",
        ),
        StorageError::PermissionDenied => api_error(
            StatusCode::FORBIDDEN,
            "PERMISSION_DENIED",
            "operation not permitted",
        ),
        StorageError::TokenExpired => api_error(
            StatusCode::UNAUTHORIZED,
            "COMMAND_EXPIRED",
            "enrollment token expired",
        ),
        StorageError::Conflict => api_error(
            StatusCode::CONFLICT,
            "OPERATION_CONFLICT",
            "conflicting operation is already queued or running",
        ),
        StorageError::InvalidCommand => api_error(
            StatusCode::BAD_REQUEST,
            "INVALID_REQUEST",
            "invalid request",
        ),
        other => {
            tracing::error!(error=%other, "control API storage error");
            api_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "INTERNAL_ERROR",
                "internal error",
            )
        }
    }
}
fn map_maintenance(error: sqlx::Error) -> ApiError {
    if matches!(error, sqlx::Error::RowNotFound) {
        api_error(
            StatusCode::NOT_FOUND,
            "SERVICE_NOT_FOUND",
            "server not found",
        )
    } else {
        tracing::error!(%error, "maintenance storage error");
        api_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "INTERNAL_ERROR",
            "internal error",
        )
    }
}
fn map_desired_state(error: DesiredStateError) -> ApiError {
    match error {
        DesiredStateError::PermissionDenied => api_error(
            StatusCode::FORBIDDEN,
            "PERMISSION_DENIED",
            "operation not permitted",
        ),
        DesiredStateError::EnforcementUnavailable => api_error(
            StatusCode::CONFLICT,
            "ENFORCEMENT_UNAVAILABLE",
            "security/network enforcement is not enabled yet; use MONITOR mode",
        ),
        DesiredStateError::Invalid => api_error(
            StatusCode::BAD_REQUEST,
            "INVALID_REQUEST",
            "invalid desired state",
        ),
        other => {
            tracing::error!(error=%other, "desired state storage error");
            api_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "INTERNAL_ERROR",
                "internal error",
            )
        }
    }
}
fn api_error(status: StatusCode, code: &str, message: &str) -> ApiError {
    (
        status,
        Json(ErrorResponse {
            code: code.into(),
            message: message.into(),
        }),
    )
}
