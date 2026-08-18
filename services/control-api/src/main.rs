use std::{collections::HashMap, net::SocketAddr, sync::Arc};

use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use chrono::{Duration, Utc};
use common::{AuditEvent, CommandQueue, DomainEvent, Environment, Organization, Project, Server, Service, User};
use protocol::{
    validate_protocol_version, AgentHandshake, Command, CommandRequest, CommandResult, CommandStatus, ServiceState,
    SystemSnapshot,
};
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use tracing::info;
use uuid::Uuid;

mod persistence;

#[derive(Debug, Clone)]
struct Config {
    bind_addr: SocketAddr,
    database_url: Option<String>,
}

impl Config {
    fn from_env() -> Result<Self, String> {
        let bind_addr = std::env::var("ARGUS_CONTROL_API_BIND")
            .unwrap_or_else(|_| "0.0.0.0:8080".to_string())
            .parse::<SocketAddr>()
            .map_err(|e| format!("invalid ARGUS_CONTROL_API_BIND: {e}"))?;

        let database_url = std::env::var("DATABASE_URL").ok();
        if let Some(url) = &database_url
            && !url.starts_with("postgres://")
            && !url.starts_with("postgresql://")
        {
            return Err("DATABASE_URL must be a PostgreSQL URL".to_string());
        }

        Ok(Self {
            bind_addr,
            database_url,
        })
    }
}

#[derive(Clone)]
struct AppState {
    inner: Arc<Mutex<InnerState>>,
}

#[derive(Default)]
struct InnerState {
    organizations: HashMap<Uuid, Organization>,
    users: HashMap<Uuid, User>,
    projects: HashMap<Uuid, Project>,
    environments: HashMap<Uuid, Environment>,
    servers: HashMap<Uuid, Server>,
    services: HashMap<Uuid, Service>,
    queue: CommandQueue,
    commands: HashMap<Uuid, Command>,
    audit_events: Vec<AuditEvent>,
    events: Vec<DomainEvent>,
    enrollment_tokens: HashMap<String, EnrollmentToken>,
    agents: HashMap<Uuid, AgentState>,
}

#[derive(Debug, Clone)]
struct EnrollmentToken {
    token: String,
    server_id: Uuid,
    organization_id: Uuid,
    expires_at: chrono::DateTime<Utc>,
}

#[derive(Debug, Clone)]
struct AgentState {
    server_id: Uuid,
    organization_id: Uuid,
    agent_id: Uuid,
    auth_token: String,
    connected: bool,
    last_heartbeat: chrono::DateTime<Utc>,
    handshake: AgentHandshake,
    snapshot: Option<SystemSnapshot>,
    services: Vec<ServiceState>,
}

#[derive(Debug, Deserialize)]
struct CreateEnrollmentTokenRequest {
    server_id: Uuid,
    organization_id: Uuid,
    ttl_seconds: i64,
}

#[derive(Debug, Serialize)]
struct CreateEnrollmentTokenResponse {
    token: String,
    expires_at: chrono::DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
struct CompleteEnrollmentRequest {
    token: String,
    handshake: AgentHandshake,
}

#[derive(Debug, Serialize)]
struct CompleteEnrollmentResponse {
    auth_token: String,
}

#[derive(Debug, Deserialize)]
struct HeartbeatRequest {
    snapshot: SystemSnapshot,
    services: Vec<ServiceState>,
}

#[derive(Debug, Serialize)]
struct QueueCommandResponse {
    command_id: Uuid,
    status: CommandStatus,
}

#[derive(Debug, Serialize)]
struct ServerView {
    server_id: Uuid,
    hostname: String,
    online: bool,
    os: String,
    agent_version: String,
    cpu_percent: f32,
    ram_percent: f32,
    disk_percent: f32,
    load: f64,
    uptime_seconds: u64,
    services: Vec<ServiceState>,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();
    let config = Config::from_env().expect("invalid control-api configuration");
    if let Some(database_url) = &config.database_url {
        persistence::bootstrap_postgres(database_url)
            .await
            .expect("failed to bootstrap PostgreSQL");
    }

    let state = AppState {
        inner: Arc::new(Mutex::new(InnerState {
            queue: CommandQueue::new(),
            ..Default::default()
        })),
    };

    seed(&state).await;

    let app = Router::new()
        .route("/health", get(health))
        .route("/servers", get(list_servers))
        .route("/servers/:server_id", get(get_server))
        .route("/commands", post(queue_command))
        .route("/agent/commands/next", post(next_command))
        .route("/agent/commands/result", post(command_result))
        .route("/enrollment/tokens", post(create_enrollment_token))
        .route("/enrollment/complete", post(complete_enrollment))
        .route("/agent/heartbeat", post(heartbeat))
        .with_state(state);

    info!(bind_addr=%config.bind_addr, has_database_url=config.database_url.is_some(), "starting control api");
    let listener = tokio::net::TcpListener::bind(config.bind_addr)
        .await
        .expect("bind tcp listener");
    axum::serve(listener, app).await.expect("serve api");
}

async fn seed(state: &AppState) {
    let mut inner = state.inner.lock().await;
    let org_id = Uuid::new_v4();
    let project_id = Uuid::new_v4();
    let environment_id = Uuid::new_v4();
    let server_id = Uuid::new_v4();
    let user_id = Uuid::new_v4();
    let service_id = Uuid::new_v4();

    inner.organizations.insert(
        org_id,
        Organization {
            id: org_id,
            name: "Argus".to_string(),
        },
    );
    inner.users.insert(
        user_id,
        User {
            id: user_id,
            organization_id: org_id,
            email: "admin@example.com".to_string(),
        },
    );
    inner.projects.insert(
        project_id,
        Project {
            id: project_id,
            organization_id: org_id,
            name: "Argus".to_string(),
            client_id: None,
        },
    );
    inner.environments.insert(
        environment_id,
        Environment {
            id: environment_id,
            organization_id: org_id,
            project_id,
            name: "Production".to_string(),
        },
    );
    inner.servers.insert(
        server_id,
        Server {
            id: server_id,
            organization_id: org_id,
            project_id,
            environment_id,
            hostname: "production-01".to_string(),
        },
    );
    inner.services.insert(
        service_id,
        Service {
            id: service_id,
            organization_id: org_id,
            project_id,
            environment_id,
            server_id,
            name: "nginx.service".to_string(),
            service_type: "systemd".to_string(),
            status: "unknown".to_string(),
        },
    );
}

async fn health() -> impl IntoResponse {
    (StatusCode::OK, "ok")
}

async fn create_enrollment_token(
    State(state): State<AppState>,
    Json(req): Json<CreateEnrollmentTokenRequest>,
) -> Result<Json<CreateEnrollmentTokenResponse>, (StatusCode, Json<ErrorResponse>)> {
    if req.ttl_seconds <= 0 {
        return Err(error(StatusCode::BAD_REQUEST, "COMMAND_EXPIRED", "invalid token ttl"));
    }

    let mut inner = state.inner.lock().await;
    if !inner.servers.contains_key(&req.server_id) {
        return Err(error(
            StatusCode::NOT_FOUND,
            "SERVICE_NOT_FOUND",
            "server not found",
        ));
    }

    let token = Uuid::new_v4().to_string();
    let expires_at = Utc::now() + Duration::seconds(req.ttl_seconds);
    inner.enrollment_tokens.insert(
        token.clone(),
        EnrollmentToken {
            token: token.clone(),
            server_id: req.server_id,
            organization_id: req.organization_id,
            expires_at,
        },
    );

    Ok(Json(CreateEnrollmentTokenResponse { token, expires_at }))
}

async fn complete_enrollment(
    State(state): State<AppState>,
    Json(req): Json<CompleteEnrollmentRequest>,
) -> Result<Json<CompleteEnrollmentResponse>, (StatusCode, Json<ErrorResponse>)> {
    validate_protocol_version(&req.handshake.protocol_version)
        .map_err(|_| error(StatusCode::BAD_REQUEST, "CAPABILITY_UNAVAILABLE", "protocol mismatch"))?;

    let mut inner = state.inner.lock().await;
    let Some(token) = inner.enrollment_tokens.remove(&req.token) else {
        return Err(error(
            StatusCode::UNAUTHORIZED,
            "PERMISSION_DENIED",
            "invalid enrollment token",
        ));
    };

    if token.expires_at < Utc::now() {
        return Err(error(
            StatusCode::UNAUTHORIZED,
            "COMMAND_EXPIRED",
            "enrollment token expired",
        ));
    }

    if token.server_id != req.handshake.server_id {
        return Err(error(
            StatusCode::UNAUTHORIZED,
            "PERMISSION_DENIED",
            "token does not match server",
        ));
    }

    let auth_token = format!("argus-agent-{}", Uuid::new_v4());

    inner.agents.insert(
        req.handshake.server_id,
        AgentState {
            server_id: req.handshake.server_id,
            organization_id: token.organization_id,
            agent_id: req.handshake.agent_id,
            auth_token: auth_token.clone(),
            connected: true,
            last_heartbeat: Utc::now(),
            handshake: req.handshake,
            snapshot: None,
            services: vec![],
        },
    );
    inner.events.push(DomainEvent::ServerConnected {
        server_id: token.server_id,
    });

    Ok(Json(CompleteEnrollmentResponse { auth_token }))
}

async fn heartbeat(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<HeartbeatRequest>,
) -> Result<StatusCode, (StatusCode, Json<ErrorResponse>)> {
    let token = bearer_token(&headers)?;
    let mut inner = state.inner.lock().await;

    let Some(agent) = inner.agents.values_mut().find(|a| a.auth_token == token) else {
        return Err(error(
            StatusCode::UNAUTHORIZED,
            "AGENT_DISCONNECTED",
            "agent identity not found",
        ));
    };

    agent.connected = true;
    agent.last_heartbeat = Utc::now();
    agent.snapshot = Some(req.snapshot);
    agent.services = req.services;
    Ok(StatusCode::NO_CONTENT)
}

async fn queue_command(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<CommandRequest>,
) -> Result<Json<QueueCommandResponse>, (StatusCode, Json<ErrorResponse>)> {
    authorize_web_request(&headers)?;

    let mut inner = state.inner.lock().await;
    if !inner.servers.contains_key(&req.server_id) {
        return Err(error(
            StatusCode::NOT_FOUND,
            "SERVICE_NOT_FOUND",
            "server not found",
        ));
    }

    let command = inner
        .queue
        .enqueue(req, Utc::now())
        .map_err(|e| match e {
            common::QueueError::Duplicate(_) => {
                error(StatusCode::CONFLICT, "OPERATION_CONFLICT", "duplicate idempotency key")
            }
            common::QueueError::Conflict { .. } => {
                error(StatusCode::CONFLICT, "OPERATION_CONFLICT", "operation conflict")
            }
            common::QueueError::InvalidTtl => {
                error(StatusCode::BAD_REQUEST, "COMMAND_EXPIRED", "invalid ttl")
            }
            common::QueueError::NotFound => error(StatusCode::NOT_FOUND, "SERVICE_NOT_FOUND", "not found"),
        })?;
    inner.commands.insert(command.id, command.clone());

    inner.audit_events.push(AuditEvent {
        id: Uuid::new_v4(),
        organization_id: inner
            .servers
            .get(&command.server_id)
            .map(|s| s.organization_id)
            .unwrap_or_default(),
        actor: "web-user".to_string(),
        resource: command.server_id.to_string(),
        action: "server.command.create".to_string(),
        request_id: Uuid::new_v4().to_string(),
        result: "QUEUED".to_string(),
        source: "web".to_string(),
        timestamp: Utc::now(),
    });

    Ok(Json(QueueCommandResponse {
        command_id: command.id,
        status: command.status,
    }))
}

async fn next_command(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Option<Command>>, (StatusCode, Json<ErrorResponse>)> {
    let token = bearer_token(&headers)?;
    let mut inner = state.inner.lock().await;
    let Some(agent) = inner.agents.values().find(|a| a.auth_token == token) else {
        return Err(error(
            StatusCode::UNAUTHORIZED,
            "AGENT_DISCONNECTED",
            "agent disconnected",
        ));
    };

    let command = inner.queue.next_for_server(agent.server_id, Utc::now());
    if let Some(command) = &command {
        let _ = inner.queue.mark_running(command.id);
        inner.events.push(DomainEvent::ServerCommandStarted {
            server_id: command.server_id,
            command_id: command.id,
        });
        inner.commands.insert(command.id, command.clone());
    }

    Ok(Json(command))
}

async fn command_result(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(result): Json<CommandResult>,
) -> Result<StatusCode, (StatusCode, Json<ErrorResponse>)> {
    let token = bearer_token(&headers)?;
    let mut inner = state.inner.lock().await;

    let Some(_agent) = inner.agents.values().find(|a| a.auth_token == token) else {
        return Err(error(
            StatusCode::UNAUTHORIZED,
            "AGENT_DISCONNECTED",
            "agent disconnected",
        ));
    };

    let command = inner
        .commands
        .get(&result.command_id)
        .cloned()
        .ok_or_else(|| error(StatusCode::NOT_FOUND, "SERVICE_NOT_FOUND", "command not found"))?;

    inner
        .queue
        .complete(result.clone())
        .map_err(|_| error(StatusCode::NOT_FOUND, "SERVICE_NOT_FOUND", "command not found"))?;

    let event = match result.status {
        CommandStatus::SUCCEEDED => DomainEvent::ServerCommandCompleted {
            server_id: command.server_id,
            command_id: command.id,
        },
        _ => DomainEvent::ServerCommandFailed {
            server_id: command.server_id,
            command_id: command.id,
        },
    };
    inner.events.push(event);

    inner.audit_events.push(AuditEvent {
        id: Uuid::new_v4(),
        organization_id: inner
            .servers
            .get(&command.server_id)
            .map(|s| s.organization_id)
            .unwrap_or_default(),
        actor: "agent".to_string(),
        resource: command.server_id.to_string(),
        action: "server.command.result".to_string(),
        request_id: Uuid::new_v4().to_string(),
        result: format!("{:?}", result.status),
        source: "agent".to_string(),
        timestamp: Utc::now(),
    });

    Ok(StatusCode::NO_CONTENT)
}

async fn list_servers(State(state): State<AppState>) -> Json<Vec<ServerView>> {
    let inner = state.inner.lock().await;
    let mut views = Vec::new();

    for server in inner.servers.values() {
        let agent = inner.agents.get(&server.id);
        let snapshot = agent.and_then(|a| a.snapshot.clone());
        let services = agent
            .map(|a| a.services.clone())
            .unwrap_or_else(|| {
                inner
                    .services
                    .values()
                    .filter(|service| service.server_id == server.id)
                    .map(|service| ServiceState {
                        name: service.name.clone(),
                        status: service.status.clone(),
                    })
                    .collect()
            });

        views.push(ServerView {
            server_id: server.id,
            hostname: server.hostname.clone(),
            online: agent
                .map(|a| a.connected && a.last_heartbeat > Utc::now() - Duration::seconds(60))
                .unwrap_or(false),
            os: snapshot
                .as_ref()
                .map(|s| s.os.clone())
                .unwrap_or_else(|| "unknown".to_string()),
            agent_version: agent
                .map(|a| a.handshake.agent_version.clone())
                .unwrap_or_else(|| "unknown".to_string()),
            cpu_percent: snapshot.as_ref().map(|s| s.cpu_percent).unwrap_or_default(),
            ram_percent: snapshot.as_ref().map(|s| s.ram_percent).unwrap_or_default(),
            disk_percent: snapshot.as_ref().map(|s| s.disk_percent).unwrap_or_default(),
            load: snapshot.as_ref().map(|s| s.load).unwrap_or_default(),
            uptime_seconds: snapshot
                .as_ref()
                .map(|s| s.uptime_seconds)
                .unwrap_or_default(),
            services,
        });
    }

    Json(views)
}

async fn get_server(
    State(state): State<AppState>,
    Path(server_id): Path<Uuid>,
) -> Result<Json<ServerView>, (StatusCode, Json<ErrorResponse>)> {
    let servers = list_servers(State(state)).await.0;
    servers
        .into_iter()
        .find(|server| server.server_id == server_id)
        .map(Json)
        .ok_or_else(|| error(StatusCode::NOT_FOUND, "SERVICE_NOT_FOUND", "server not found"))
}

#[derive(Debug, Serialize)]
struct ErrorResponse {
    code: String,
    message: String,
}

fn error(status: StatusCode, code: &str, message: &str) -> (StatusCode, Json<ErrorResponse>) {
    (
        status,
        Json(ErrorResponse {
            code: code.to_string(),
            message: message.to_string(),
        }),
    )
}

fn authorize_web_request(headers: &HeaderMap) -> Result<(), (StatusCode, Json<ErrorResponse>)> {
    let Some(org) = headers.get("x-org-id") else {
        return Err(error(
            StatusCode::UNAUTHORIZED,
            "PERMISSION_DENIED",
            "missing organization context",
        ));
    };

    if org.is_empty() {
        return Err(error(
            StatusCode::UNAUTHORIZED,
            "PERMISSION_DENIED",
            "invalid organization",
        ));
    }

    Ok(())
}

fn bearer_token(headers: &HeaderMap) -> Result<String, (StatusCode, Json<ErrorResponse>)> {
    let Some(value) = headers.get("authorization") else {
        return Err(error(
            StatusCode::UNAUTHORIZED,
            "PERMISSION_DENIED",
            "missing authorization header",
        ));
    };

    let Ok(value) = value.to_str() else {
        return Err(error(
            StatusCode::UNAUTHORIZED,
            "PERMISSION_DENIED",
            "invalid authorization header",
        ));
    };

    let Some(token) = value.strip_prefix("Bearer ") else {
        return Err(error(
            StatusCode::UNAUTHORIZED,
            "PERMISSION_DENIED",
            "invalid bearer token",
        ));
    };

    Ok(token.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use protocol::{CommandType, RiskLevel};

    #[tokio::test]
    async fn enrollment_token_expires() {
        let state = AppState {
            inner: Arc::new(Mutex::new(InnerState {
                queue: CommandQueue::new(),
                ..Default::default()
            })),
        };
        seed(&state).await;

        let server_id = state
            .inner
            .lock()
            .await
            .servers
            .keys()
            .next()
            .copied()
            .expect("server");

        let req = CreateEnrollmentTokenRequest {
            server_id,
            organization_id: Uuid::new_v4(),
            ttl_seconds: 1,
        };
        let Json(token_response) = create_enrollment_token(State(state.clone()), Json(req))
            .await
            .expect("token");

        {
            let mut guard = state.inner.lock().await;
            if let Some(token) = guard.enrollment_tokens.get_mut(&token_response.token) {
                token.expires_at = Utc::now() - Duration::seconds(1);
            }
        }

        let handshake = AgentHandshake {
            agent_id: Uuid::new_v4(),
            server_id,
            agent_version: "0.1.0".to_string(),
            protocol_version: protocol::PROTOCOL_VERSION.to_string(),
            hostname: "production-01".to_string(),
            os: "ubuntu".to_string(),
            architecture: "x86_64".to_string(),
            capabilities: vec![],
        };

        let err = complete_enrollment(
            State(state),
            Json(CompleteEnrollmentRequest {
                token: token_response.token,
                handshake,
            }),
        )
        .await
        .expect_err("must fail");

        let (status, Json(body)) = err;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(body.code, "COMMAND_EXPIRED");
    }

    #[tokio::test]
    async fn duplicate_command_idempotency_is_rejected() {
        let state = AppState {
            inner: Arc::new(Mutex::new(InnerState {
                queue: CommandQueue::new(),
                ..Default::default()
            })),
        };
        seed(&state).await;

        let server_id = state
            .inner
            .lock()
            .await
            .servers
            .keys()
            .next()
            .copied()
            .expect("server");

        let req = CommandRequest {
            server_id,
            command_type: CommandType::ServiceRestart {
                service: "nginx.service".to_string(),
            },
            ttl_seconds: 60,
            idempotency_key: "same".to_string(),
            risk_level: RiskLevel::MEDIUM,
        };

        let mut headers = HeaderMap::new();
        headers.insert("x-org-id", "demo".parse().expect("header"));

        queue_command(State(state.clone()), headers.clone(), Json(req.clone()))
            .await
            .expect("first request");
        let err = queue_command(State(state), headers, Json(req))
            .await
            .expect_err("duplicate should fail");

        let (status, Json(body)) = err;
        assert_eq!(status, StatusCode::CONFLICT);
        assert_eq!(body.code, "OPERATION_CONFLICT");
    }
}
