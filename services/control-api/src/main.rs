use axum::{extract::{Path, State}, http::{HeaderMap, StatusCode}, response::{IntoResponse, Response}, routing::{get, post}, Json, Router};
use protocol::{validate_protocol_version, CommandRequest, CommandResult, EnrollmentRequest, EnrollmentResponse, HeartbeatRequest};
use serde::{Deserialize, Serialize};
use std::{net::SocketAddr, sync::Arc};
use tracing::info;
use uuid::Uuid;

mod persistence;
use persistence::{Storage, StorageError, WebIdentity};

#[derive(Clone)]
struct AppState { storage: Storage, web_api_token: Arc<String> }

#[derive(Debug)]
struct Config { bind_addr: SocketAddr, database_url: String, web_api_token: String }
impl Config {
    fn from_env() -> Result<Self, String> {
        let bind_addr = std::env::var("ARGUS_CONTROL_API_BIND").unwrap_or_else(|_| "0.0.0.0:8080".into()).parse().map_err(|e| format!("invalid bind address: {e}"))?;
        let database_url = std::env::var("DATABASE_URL").map_err(|_| "DATABASE_URL is required".to_string())?;
        if !database_url.starts_with("postgres://") && !database_url.starts_with("postgresql://") { return Err("DATABASE_URL must be PostgreSQL".into()); }
        let web_api_token = std::env::var("ARGUS_WEB_API_TOKEN").map_err(|_| "ARGUS_WEB_API_TOKEN is required".to_string())?;
        if web_api_token.len() < 32 { return Err("ARGUS_WEB_API_TOKEN must be at least 32 characters".into()); }
        Ok(Self { bind_addr, database_url, web_api_token })
    }
}

#[derive(Debug, Serialize)]
struct ErrorResponse { code: String, message: String }
type ApiError = (StatusCode, Json<ErrorResponse>);

#[derive(Debug, Deserialize)]
struct CreateServerRequest { project_id: Uuid, environment_id: Uuid, hostname: String }
#[derive(Debug, Serialize)]
struct CreateServerResponse { server_id: Uuid }
#[derive(Debug, Deserialize)]
struct CreateEnrollmentTokenRequest { server_id: Uuid, ttl_seconds: i64 }
#[derive(Debug, Serialize)]
struct CreateEnrollmentTokenResponse { token: String, expires_at: chrono::DateTime<chrono::Utc> }

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt().with_env_filter(tracing_subscriber::EnvFilter::from_default_env()).init();
    let config = Config::from_env().map_err(anyhow::Error::msg)?;
    let storage = Storage::connect(&config.database_url).await?;
    let state = AppState { storage, web_api_token: Arc::new(config.web_api_token) };
    let app = Router::new()
        .route("/health", get(health))
        .route("/servers", get(list_servers).post(create_server))
        .route("/servers/:server_id", get(get_server))
        .route("/servers/:server_id/commands", get(command_history))
        .route("/commands", post(queue_command))
        .route("/enrollment/tokens", post(create_enrollment_token))
        .route("/enrollment/complete", post(complete_enrollment))
        .route("/agent/heartbeat", post(heartbeat))
        .route("/agent/commands/next", post(next_command))
        .route("/agent/commands/result", post(command_result))
        .with_state(state);
    info!(bind_addr=%config.bind_addr, "starting persistent Argus control API");
    let listener = tokio::net::TcpListener::bind(config.bind_addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

async fn health() -> &'static str { "ok" }

async fn web_identity(state: &AppState, headers: &HeaderMap) -> Result<WebIdentity, ApiError> {
    let bearer = bearer_token(headers)?;
    if bearer != state.web_api_token.as_str() { return Err(api_error(StatusCode::UNAUTHORIZED, "PERMISSION_DENIED", "invalid web backend credential")); }
    let user_id = uuid_header(headers, "x-argus-user-id")?;
    let organization_id = uuid_header(headers, "x-argus-org-id")?;
    let identity = WebIdentity { user_id, organization_id };
    state.storage.verify_web_identity(identity).await.map_err(map_storage)?;
    Ok(identity)
}

async fn list_servers(State(state): State<AppState>, headers: HeaderMap) -> Result<Json<Vec<persistence::ServerView>>, ApiError> {
    let identity = web_identity(&state, &headers).await?;
    Ok(Json(state.storage.list_servers(identity).await.map_err(map_storage)?))
}

async fn get_server(State(state): State<AppState>, headers: HeaderMap, Path(server_id): Path<Uuid>) -> Result<Json<persistence::ServerView>, ApiError> {
    let identity = web_identity(&state, &headers).await?;
    Ok(Json(state.storage.get_server(identity, server_id).await.map_err(map_storage)?))
}

async fn create_server(State(state): State<AppState>, headers: HeaderMap, Json(req): Json<CreateServerRequest>) -> Result<Json<CreateServerResponse>, ApiError> {
    let identity = web_identity(&state, &headers).await?;
    let hostname = req.hostname.trim();
    if hostname.is_empty() || hostname.len() > 255 { return Err(api_error(StatusCode::BAD_REQUEST, "INVALID_REQUEST", "invalid hostname")); }
    let server_id = state.storage.create_server(identity, req.project_id, req.environment_id, hostname).await.map_err(map_storage)?;
    Ok(Json(CreateServerResponse { server_id }))
}

async fn create_enrollment_token(State(state): State<AppState>, headers: HeaderMap, Json(req): Json<CreateEnrollmentTokenRequest>) -> Result<Json<CreateEnrollmentTokenResponse>, ApiError> {
    let identity = web_identity(&state, &headers).await?;
    let (token, expires_at) = state.storage.create_enrollment_token(identity, req.server_id, req.ttl_seconds).await.map_err(map_storage)?;
    Ok(Json(CreateEnrollmentTokenResponse { token, expires_at }))
}

async fn complete_enrollment(State(state): State<AppState>, Json(req): Json<EnrollmentRequest>) -> Result<Json<EnrollmentResponse>, ApiError> {
    validate_protocol_version(&req.handshake.protocol_version).map_err(|_| api_error(StatusCode::BAD_REQUEST, "CAPABILITY_UNAVAILABLE", "unsupported protocol version"))?;
    let credential = state.storage.complete_enrollment(&req.token, &req.handshake).await.map_err(map_storage)?;
    Ok(Json(EnrollmentResponse { credential }))
}

async fn heartbeat(State(state): State<AppState>, headers: HeaderMap, Json(req): Json<HeartbeatRequest>) -> Result<StatusCode, ApiError> {
    let credential = bearer_token(&headers)?;
    state.storage.heartbeat(credential, &req).await.map_err(map_storage)?;
    Ok(StatusCode::NO_CONTENT)
}

async fn queue_command(State(state): State<AppState>, headers: HeaderMap, Json(req): Json<CommandRequest>) -> Result<Json<protocol::Command>, ApiError> {
    let identity = web_identity(&state, &headers).await?;
    Ok(Json(state.storage.queue_command(identity, req).await.map_err(map_storage)?))
}

async fn next_command(State(state): State<AppState>, headers: HeaderMap) -> Result<Response, ApiError> {
    let credential = bearer_token(&headers)?;
    match state.storage.claim_next_command(credential).await.map_err(map_storage)? {
        Some(command) => Ok(Json(command).into_response()),
        None => Ok(StatusCode::NO_CONTENT.into_response()),
    }
}

async fn command_result(State(state): State<AppState>, headers: HeaderMap, Json(result): Json<CommandResult>) -> Result<StatusCode, ApiError> {
    let credential = bearer_token(&headers)?;
    state.storage.submit_command_result(credential, result).await.map_err(map_storage)?;
    Ok(StatusCode::NO_CONTENT)
}

async fn command_history(State(state): State<AppState>, headers: HeaderMap, Path(server_id): Path<Uuid>) -> Result<Json<Vec<persistence::CommandHistoryItem>>, ApiError> {
    let identity = web_identity(&state, &headers).await?;
    Ok(Json(state.storage.command_history(identity, server_id).await.map_err(map_storage)?))
}

fn bearer_token(headers: &HeaderMap) -> Result<&str, ApiError> {
    let value = headers.get("authorization").and_then(|v| v.to_str().ok()).ok_or_else(|| api_error(StatusCode::UNAUTHORIZED, "PERMISSION_DENIED", "missing authorization"))?;
    value.strip_prefix("Bearer ").filter(|v| !v.is_empty()).ok_or_else(|| api_error(StatusCode::UNAUTHORIZED, "PERMISSION_DENIED", "invalid authorization"))
}

fn uuid_header(headers: &HeaderMap, name: &str) -> Result<Uuid, ApiError> {
    headers.get(name).and_then(|v| v.to_str().ok()).and_then(|v| v.parse().ok()).ok_or_else(|| api_error(StatusCode::UNAUTHORIZED, "PERMISSION_DENIED", &format!("missing or invalid {name}")))
}

fn map_storage(error: StorageError) -> ApiError {
    match error {
        StorageError::NotFound => api_error(StatusCode::NOT_FOUND, "SERVICE_NOT_FOUND", "resource not found"),
        StorageError::PermissionDenied => api_error(StatusCode::FORBIDDEN, "PERMISSION_DENIED", "operation not permitted"),
        StorageError::TokenExpired => api_error(StatusCode::UNAUTHORIZED, "COMMAND_EXPIRED", "enrollment token expired"),
        StorageError::Conflict => api_error(StatusCode::CONFLICT, "OPERATION_CONFLICT", "conflicting operation is already queued or running"),
        StorageError::InvalidCommand => api_error(StatusCode::BAD_REQUEST, "INVALID_REQUEST", "invalid request"),
        other => { tracing::error!(error=%other, "control API storage error"); api_error(StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL_ERROR", "internal error") }
    }
}

fn api_error(status: StatusCode, code: &str, message: &str) -> ApiError {
    (status, Json(ErrorResponse { code: code.into(), message: message.into() }))
}
