use chrono::{DateTime, Duration, Utc};
use protocol::{
    AgentHandshake, Capability, Command, CommandRequest, CommandResult, CommandStatus,
    HeartbeatRequest, RiskLevel, ServiceState, SystemSnapshot,
};
use serde::Serialize;
use sha2::{Digest, Sha256};
use sqlx::{PgPool, Row};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct Storage {
    pool: PgPool,
}

#[derive(Debug, Clone, Copy)]
pub struct WebIdentity {
    pub user_id: Uuid,
    pub organization_id: Uuid,
}

#[derive(Debug, Clone)]
pub struct AgentIdentity {
    pub server_id: Uuid,
    pub organization_id: Uuid,
    pub agent_id: Uuid,
}

#[derive(Debug, Clone, Serialize)]
pub struct ServerView {
    pub server_id: Uuid,
    pub project_id: Uuid,
    pub environment_id: Uuid,
    pub hostname: String,
    pub online: bool,
    pub last_heartbeat: Option<DateTime<Utc>>,
    pub snapshot: Option<SystemSnapshot>,
    pub services: Vec<ServiceState>,
    pub capabilities: Vec<Capability>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CommandHistoryItem {
    pub command: Command,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
    pub actor_user_id: Option<Uuid>,
    pub phase: Option<String>,
    pub output: Option<String>,
    pub output_truncated: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct MetricSample {
    pub captured_at: DateTime<Utc>,
    pub cpu_percent: f32,
    pub ram_percent: f32,
    pub disk_percent: f32,
    pub load: f64,
}

#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("not found")]
    NotFound,
    #[error("permission denied")]
    PermissionDenied,
    #[error("token expired")]
    TokenExpired,
    #[error("operation conflict")]
    Conflict,
    #[error("invalid command")]
    InvalidCommand,
    #[error(transparent)]
    Sql(#[from] sqlx::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

impl Storage {
    pub async fn connect(database_url: &str) -> Result<Self, sqlx::Error> {
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(10)
            .connect(database_url)
            .await?;
        sqlx::migrate!("./migrations").run(&pool).await?;
        Ok(Self { pool })
    }

    pub async fn verify_web_identity(&self, identity: WebIdentity) -> Result<(), StorageError> {
        let exists: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM users WHERE id=$1 AND organization_id=$2)",
        )
        .bind(identity.user_id)
        .bind(identity.organization_id)
        .fetch_one(&self.pool)
        .await?;

        if exists {
            Ok(())
        } else {
            Err(StorageError::PermissionDenied)
        }
    }

    pub async fn create_server(
        &self,
        identity: WebIdentity,
        project_id: Uuid,
        environment_id: Uuid,
        hostname: &str,
    ) -> Result<Uuid, StorageError> {
        self.verify_web_identity(identity).await?;
        let valid: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM projects p JOIN environments e ON e.project_id=p.id WHERE p.id=$1 AND e.id=$2 AND p.organization_id=$3 AND e.organization_id=$3)",
        )
        .bind(project_id)
        .bind(environment_id)
        .bind(identity.organization_id)
        .fetch_one(&self.pool)
        .await?;
        if !valid {
            return Err(StorageError::PermissionDenied);
        }

        let id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO servers(id,organization_id,project_id,environment_id,hostname) VALUES($1,$2,$3,$4,$5)",
        )
        .bind(id)
        .bind(identity.organization_id)
        .bind(project_id)
        .bind(environment_id)
        .bind(hostname)
        .execute(&self.pool)
        .await?;

        Ok(id)
    }

    pub async fn create_enrollment_token(
        &self,
        identity: WebIdentity,
        server_id: Uuid,
        ttl_seconds: i64,
    ) -> Result<(String, DateTime<Utc>), StorageError> {
        if !(1..=3600).contains(&ttl_seconds) {
            return Err(StorageError::InvalidCommand);
        }
        self.verify_web_identity(identity).await?;
        let allowed: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM servers WHERE id=$1 AND organization_id=$2)",
        )
        .bind(server_id)
        .bind(identity.organization_id)
        .fetch_one(&self.pool)
        .await?;
        if !allowed {
            return Err(StorageError::PermissionDenied);
        }

        let token = random_secret();
        let expires_at = Utc::now() + Duration::seconds(ttl_seconds);
        sqlx::query(
            "INSERT INTO enrollment_tokens(token_hash,server_id,organization_id,expires_at,created_by) VALUES($1,$2,$3,$4,$5)",
        )
        .bind(hash_secret(&token))
        .bind(server_id)
        .bind(identity.organization_id)
        .bind(expires_at)
        .bind(identity.user_id)
        .execute(&self.pool)
        .await?;

        self.audit(
            identity.organization_id,
            identity.user_id.to_string(),
            server_id.to_string(),
            "server.enrollment_token.create",
            "SUCCEEDED",
            "web",
        )
        .await?;
        Ok((token, expires_at))
    }

    pub async fn complete_enrollment(
        &self,
        token: &str,
        handshake: &AgentHandshake,
    ) -> Result<String, StorageError> {
        let mut tx = self.pool.begin().await?;
        let row = sqlx::query(
            "DELETE FROM enrollment_tokens WHERE token_hash=$1 RETURNING server_id,organization_id,expires_at",
        )
        .bind(hash_secret(token))
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(StorageError::PermissionDenied)?;

        let server_id: Uuid = row.get("server_id");
        let organization_id: Uuid = row.get("organization_id");
        let expires_at: DateTime<Utc> = row.get("expires_at");
        if expires_at < Utc::now() {
            tx.rollback().await?;
            return Err(StorageError::TokenExpired);
        }
        if server_id != handshake.server_id {
            tx.rollback().await?;
            return Err(StorageError::PermissionDenied);
        }

        let credential = format!("argus_{}", random_secret());
        sqlx::query(
            "INSERT INTO agents(server_id,organization_id,agent_id,credential_hash,agent_version,protocol_version,capabilities,last_seen_at) VALUES($1,$2,$3,$4,$5,$6,$7,NOW()) ON CONFLICT(server_id) DO UPDATE SET agent_id=EXCLUDED.agent_id,credential_hash=EXCLUDED.credential_hash,agent_version=EXCLUDED.agent_version,protocol_version=EXCLUDED.protocol_version,capabilities=EXCLUDED.capabilities,last_seen_at=NOW(),snapshot=NULL,services='[]'::jsonb",
        )
        .bind(server_id)
        .bind(organization_id)
        .bind(handshake.agent_id)
        .bind(hash_secret(&credential))
        .bind(&handshake.agent_version)
        .bind(&handshake.protocol_version)
        .bind(serde_json::to_value(&handshake.capabilities)?)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;

        self.audit(
            organization_id,
            handshake.agent_id.to_string(),
            server_id.to_string(),
            "agent.enrolled",
            "SUCCEEDED",
            "agent",
        )
        .await?;
        self.emit_event(
            organization_id,
            "server.connected",
            server_id,
            serde_json::json!({"reason":"enrollment"}),
        )
        .await?;
        Ok(credential)
    }

    pub async fn authenticate_agent(
        &self,
        credential: &str,
    ) -> Result<AgentIdentity, StorageError> {
        let row = sqlx::query(
            "SELECT server_id,organization_id,agent_id FROM agents WHERE credential_hash=$1",
        )
        .bind(hash_secret(credential))
        .fetch_optional(&self.pool)
        .await?
        .ok_or(StorageError::PermissionDenied)?;

        Ok(AgentIdentity {
            server_id: row.get("server_id"),
            organization_id: row.get("organization_id"),
            agent_id: row.get("agent_id"),
        })
    }

    pub async fn heartbeat(
        &self,
        credential: &str,
        heartbeat: &HeartbeatRequest,
    ) -> Result<AgentIdentity, StorageError> {
        let identity = self.authenticate_agent(credential).await?;
        if heartbeat.snapshot.server_id != identity.server_id {
            return Err(StorageError::PermissionDenied);
        }

        let previous: Option<DateTime<Utc>> =
            sqlx::query_scalar("SELECT last_seen_at FROM agents WHERE server_id=$1")
                .bind(identity.server_id)
                .fetch_optional(&self.pool)
                .await?;
        let reconnect = previous.is_some_and(|seen| seen < Utc::now() - Duration::seconds(90));

        if reconnect {
            sqlx::query(
                "UPDATE commands SET status='UNKNOWN',finished_at=NOW(),error_code='AGENT_DISCONNECTED',error_message='agent disconnected while command was running' WHERE server_id=$1 AND status IN ('ACCEPTED','RUNNING')",
            )
            .bind(identity.server_id)
            .execute(&self.pool)
            .await?;
        }

        sqlx::query(
            "UPDATE agents SET last_seen_at=NOW(),agent_version=$2,snapshot=$3,services=$4 WHERE server_id=$1",
        )
        .bind(identity.server_id)
        .bind(&heartbeat.snapshot.agent_version)
        .bind(serde_json::to_value(&heartbeat.snapshot)?)
        .bind(serde_json::to_value(&heartbeat.services)?)
        .execute(&self.pool)
        .await?;

        sqlx::query(
            "INSERT INTO server_metric_samples(server_id,organization_id,captured_at,cpu_percent,ram_percent,disk_percent,load) VALUES($1,$2,$3,$4,$5,$6,$7) ON CONFLICT DO NOTHING",
        )
        .bind(identity.server_id)
        .bind(identity.organization_id)
        .bind(heartbeat.snapshot.captured_at)
        .bind(heartbeat.snapshot.cpu_percent)
        .bind(heartbeat.snapshot.ram_percent)
        .bind(heartbeat.snapshot.disk_percent)
        .bind(heartbeat.snapshot.load)
        .execute(&self.pool)
        .await?;

        if reconnect {
            self.emit_event(
                identity.organization_id,
                "server.connected",
                identity.server_id,
                serde_json::json!({"reason":"reconnect"}),
            )
            .await?;
        }
        Ok(identity)
    }

    pub async fn list_servers(
        &self,
        identity: WebIdentity,
    ) -> Result<Vec<ServerView>, StorageError> {
        self.verify_web_identity(identity).await?;
        let rows = sqlx::query(
            "SELECT s.id,s.project_id,s.environment_id,s.hostname,a.last_seen_at,a.snapshot,a.services,a.capabilities FROM servers s LEFT JOIN agents a ON a.server_id=s.id WHERE s.organization_id=$1 ORDER BY s.hostname",
        )
        .bind(identity.organization_id)
        .fetch_all(&self.pool)
        .await?;
        rows.into_iter().map(server_from_row).collect()
    }

    pub async fn get_server(
        &self,
        identity: WebIdentity,
        server_id: Uuid,
    ) -> Result<ServerView, StorageError> {
        self.verify_web_identity(identity).await?;
        let row = sqlx::query(
            "SELECT s.id,s.project_id,s.environment_id,s.hostname,a.last_seen_at,a.snapshot,a.services,a.capabilities FROM servers s LEFT JOIN agents a ON a.server_id=s.id WHERE s.id=$1 AND s.organization_id=$2",
        )
        .bind(server_id)
        .bind(identity.organization_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(StorageError::NotFound)?;
        server_from_row(row)
    }

    pub async fn queue_command(
        &self,
        identity: WebIdentity,
        request: CommandRequest,
    ) -> Result<Command, StorageError> {
        if !(1..=3600).contains(&request.ttl_seconds) {
            return Err(StorageError::InvalidCommand);
        }
        self.verify_web_identity(identity).await?;
        let mut tx = self.pool.begin().await?;

        let allowed: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM servers WHERE id=$1 AND organization_id=$2)",
        )
        .bind(request.server_id)
        .bind(identity.organization_id)
        .fetch_one(&mut *tx)
        .await?;
        if !allowed {
            tx.rollback().await?;
            return Err(StorageError::PermissionDenied);
        }

        if let Some(row) = sqlx::query(
            "SELECT id,server_id,command_type,status,idempotency_key,risk_level,created_at,expires_at FROM commands WHERE server_id=$1 AND idempotency_key=$2",
        )
        .bind(request.server_id)
        .bind(&request.idempotency_key)
        .fetch_optional(&mut *tx)
        .await?
        {
            tx.commit().await?;
            return command_from_row(row);
        }

        let conflict_group = request.command_type.conflict_group();
        let conflict: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM commands WHERE server_id=$1 AND conflict_group=$2 AND status IN ('QUEUED','ACCEPTED','RUNNING'))",
        )
        .bind(request.server_id)
        .bind(conflict_group)
        .fetch_one(&mut *tx)
        .await?;
        if conflict {
            tx.rollback().await?;
            return Err(StorageError::Conflict);
        }

        let now = Utc::now();
        let command = Command {
            id: Uuid::new_v4(),
            server_id: request.server_id,
            command_type: request.command_type,
            created_at: now,
            expires_at: now + Duration::seconds(request.ttl_seconds),
            status: CommandStatus::QUEUED,
            idempotency_key: request.idempotency_key,
            risk_level: request.risk_level,
        };
        sqlx::query(
            "INSERT INTO commands(id,server_id,command_type,status,idempotency_key,risk_level,created_at,expires_at,conflict_group,actor_user_id) VALUES($1,$2,$3,'QUEUED',$4,$5,$6,$7,$8,$9)",
        )
        .bind(command.id)
        .bind(command.server_id)
        .bind(serde_json::to_value(&command.command_type)?)
        .bind(&command.idempotency_key)
        .bind(format!("{:?}", command.risk_level))
        .bind(command.created_at)
        .bind(command.expires_at)
        .bind(conflict_group)
        .bind(identity.user_id)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;

        self.audit(
            identity.organization_id,
            identity.user_id.to_string(),
            command.server_id.to_string(),
            "server.command.create",
            "QUEUED",
            "web",
        )
        .await?;
        Ok(command)
    }

    pub async fn claim_next_command(
        &self,
        credential: &str,
    ) -> Result<Option<Command>, StorageError> {
        let agent = self.authenticate_agent(credential).await?;
        let mut tx = self.pool.begin().await?;

        sqlx::query(
            "UPDATE commands SET status='EXPIRED',finished_at=NOW(),error_code='COMMAND_EXPIRED' WHERE server_id=$1 AND status='QUEUED' AND expires_at < NOW()",
        )
        .bind(agent.server_id)
        .execute(&mut *tx)
        .await?;

        let row = sqlx::query(
            "SELECT id,server_id,command_type,status,idempotency_key,risk_level,created_at,expires_at FROM commands WHERE server_id=$1 AND status='QUEUED' AND expires_at >= NOW() ORDER BY created_at FOR UPDATE SKIP LOCKED LIMIT 1",
        )
        .bind(agent.server_id)
        .fetch_optional(&mut *tx)
        .await?;

        let Some(row) = row else {
            tx.commit().await?;
            return Ok(None);
        };
        let mut command = command_from_row(row)?;
        sqlx::query("UPDATE commands SET status='RUNNING',phase=CASE command_type->>'kind' WHEN 'packages.refresh' THEN 'REFRESHING_REPOSITORIES' WHEN 'packages.upgrade.security' THEN 'INSTALLING_SECURITY_UPDATES' WHEN 'packages.upgrade.all' THEN 'INSTALLING_PACKAGES' WHEN 'argus.update' THEN 'SCHEDULING_HOST_UPDATE' ELSE 'EXECUTING' END,started_at=NOW() WHERE id=$1")
            .bind(command.id)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        command.status = CommandStatus::RUNNING;

        self.audit(
            agent.organization_id,
            agent.agent_id.to_string(),
            agent.server_id.to_string(),
            "server.command.started",
            "RUNNING",
            "agent",
        )
        .await?;
        self.emit_event(
            agent.organization_id,
            "server.command.started",
            agent.server_id,
            serde_json::json!({"command_id":command.id}),
        )
        .await?;
        Ok(Some(command))
    }

    pub async fn submit_command_result(
        &self,
        credential: &str,
        result: CommandResult,
    ) -> Result<(), StorageError> {
        if !matches!(
            result.status,
            CommandStatus::SUCCEEDED | CommandStatus::FAILED | CommandStatus::UNKNOWN
        ) {
            return Err(StorageError::InvalidCommand);
        }

        let agent = self.authenticate_agent(credential).await?;
        let mut tx = self.pool.begin().await?;
        let server_id: Uuid =
            sqlx::query_scalar("SELECT server_id FROM commands WHERE id=$1 FOR UPDATE")
                .bind(result.command_id)
                .fetch_optional(&mut *tx)
                .await?
                .ok_or(StorageError::NotFound)?;
        if server_id != agent.server_id {
            tx.rollback().await?;
            return Err(StorageError::PermissionDenied);
        }

        let (error_code, error_message) = result
            .error
            .as_ref()
            .map(|error| {
                let mut message = error.message.clone();
                if message.len() > 4096 {
                    let mut boundary = 4096;
                    while !message.is_char_boundary(boundary) {
                        boundary -= 1;
                    }
                    message.truncate(boundary);
                    message.push_str("… See full log.");
                }
                (Some(error.code.clone()), Some(message))
            })
            .unwrap_or((None, None));
        let output = result.output.as_deref().map(|value| {
            if value.len() <= 25 * 1024 * 1024 {
                value.to_string()
            } else {
                let mut boundary = 25 * 1024 * 1024;
                while !value.is_char_boundary(boundary) {
                    boundary -= 1;
                }
                format!(
                    "{}\n[output truncated by control plane]\n",
                    &value[..boundary]
                )
            }
        });
        let output_truncated = result
            .output
            .as_ref()
            .is_some_and(|value| value.len() > 25 * 1024 * 1024);
        let phase = if result.status == CommandStatus::SUCCEEDED {
            "COMPLETE"
        } else {
            "FAILED"
        };
        sqlx::query(
            "UPDATE commands SET status=$2,phase=$3,finished_at=$4,error_code=$5,error_message=$6,output=$7,output_truncated=$8 WHERE id=$1",
        )
        .bind(result.command_id)
        .bind(format!("{:?}", result.status))
        .bind(phase)
        .bind(result.finished_at)
        .bind(error_code)
        .bind(error_message)
        .bind(output)
        .bind(output_truncated)
        .execute(&mut *tx)
        .await?;
        sqlx::query("UPDATE commands SET output=NULL,output_truncated=FALSE WHERE finished_at < NOW() - INTERVAL '30 days' AND output IS NOT NULL")
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;

        self.audit(
            agent.organization_id,
            agent.agent_id.to_string(),
            server_id.to_string(),
            "server.command.result",
            &format!("{:?}", result.status),
            "agent",
        )
        .await?;
        let event_type = match result.status {
            CommandStatus::SUCCEEDED => "server.command.completed",
            _ => "server.command.failed",
        };
        self.emit_event(
            agent.organization_id,
            event_type,
            server_id,
            serde_json::json!({"command_id":result.command_id,"status":format!("{:?}",result.status)}),
        )
        .await?;
        Ok(())
    }

    pub async fn command_history(
        &self,
        identity: WebIdentity,
        server_id: Uuid,
    ) -> Result<Vec<CommandHistoryItem>, StorageError> {
        self.verify_web_identity(identity).await?;
        let allowed: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM servers WHERE id=$1 AND organization_id=$2)",
        )
        .bind(server_id)
        .bind(identity.organization_id)
        .fetch_one(&self.pool)
        .await?;
        if !allowed {
            return Err(StorageError::PermissionDenied);
        }

        let rows = sqlx::query(
            "SELECT id,server_id,command_type,status,idempotency_key,risk_level,created_at,expires_at,started_at,finished_at,error_code,error_message,actor_user_id,phase,output,output_truncated FROM commands WHERE server_id=$1 ORDER BY created_at DESC LIMIT 50",
        )
        .bind(server_id)
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter()
            .map(|row| {
                let command = command_from_row_ref(&row)?;
                Ok(CommandHistoryItem {
                    command,
                    started_at: row.try_get("started_at")?,
                    finished_at: row.try_get("finished_at")?,
                    error_code: row.try_get("error_code")?,
                    error_message: row.try_get("error_message")?,
                    actor_user_id: row.try_get("actor_user_id")?,
                    phase: row.try_get("phase")?,
                    output: row.try_get("output")?,
                    output_truncated: row.try_get("output_truncated")?,
                })
            })
            .collect()
    }

    pub async fn metric_history(
        &self,
        identity: WebIdentity,
        server_id: Uuid,
        hours: i64,
    ) -> Result<Vec<MetricSample>, StorageError> {
        self.verify_web_identity(identity).await?;
        let hours = hours.clamp(1, 24 * 30);
        let bucket = if hours <= 24 {
            "5 seconds"
        } else if hours <= 24 * 7 {
            "1 minute"
        } else {
            "15 minutes"
        };
        let rows = sqlx::query(
            "SELECT date_bin($4::interval,m.captured_at,TIMESTAMPTZ '2000-01-01') AS captured_at,AVG(m.cpu_percent)::REAL AS cpu_percent,AVG(m.ram_percent)::REAL AS ram_percent,AVG(m.disk_percent)::REAL AS disk_percent,AVG(m.load)::DOUBLE PRECISION AS load FROM server_metric_samples m JOIN servers s ON s.id=m.server_id WHERE m.server_id=$1 AND s.organization_id=$2 AND m.captured_at >= NOW() - make_interval(hours => $3::int) GROUP BY 1 ORDER BY 1",
        )
        .bind(server_id)
        .bind(identity.organization_id)
        .bind(hours as i32)
        .bind(bucket)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(|row| MetricSample {
                captured_at: row.get("captured_at"),
                cpu_percent: row.get("cpu_percent"),
                ram_percent: row.get("ram_percent"),
                disk_percent: row.get("disk_percent"),
                load: row.get("load"),
            })
            .collect())
    }

    async fn audit(
        &self,
        organization_id: Uuid,
        actor: String,
        resource: String,
        action: &str,
        result: &str,
        source: &str,
    ) -> Result<(), StorageError> {
        sqlx::query(
            "INSERT INTO audit_events(id,organization_id,actor,resource,action,request_id,result,source,timestamp) VALUES($1,$2,$3,$4,$5,$6,$7,$8,NOW())",
        )
        .bind(Uuid::new_v4())
        .bind(organization_id)
        .bind(actor)
        .bind(resource)
        .bind(action)
        .bind(Uuid::new_v4().to_string())
        .bind(result)
        .bind(source)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn emit_event(
        &self,
        organization_id: Uuid,
        event_type: &str,
        resource_id: Uuid,
        data: serde_json::Value,
    ) -> Result<(), StorageError> {
        sqlx::query(
            "INSERT INTO domain_events(id,organization_id,event_type,resource_id,data,occurred_at) VALUES($1,$2,$3,$4,$5,NOW())",
        )
        .bind(Uuid::new_v4())
        .bind(organization_id)
        .bind(event_type)
        .bind(resource_id)
        .bind(data)
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}

fn random_secret() -> String {
    format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple())
}

fn hash_secret(secret: &str) -> String {
    hex::encode(Sha256::digest(secret.as_bytes()))
}

fn server_from_row(row: sqlx::postgres::PgRow) -> Result<ServerView, StorageError> {
    let last_heartbeat: Option<DateTime<Utc>> = row.try_get("last_seen_at")?;
    let snapshot = row
        .try_get::<Option<serde_json::Value>, _>("snapshot")?
        .map(serde_json::from_value)
        .transpose()?;
    let services = row
        .try_get::<Option<serde_json::Value>, _>("services")?
        .map(serde_json::from_value)
        .transpose()?
        .unwrap_or_default();
    let capabilities = row
        .try_get::<Option<serde_json::Value>, _>("capabilities")?
        .map(serde_json::from_value)
        .transpose()?
        .unwrap_or_default();

    Ok(ServerView {
        server_id: row.get("id"),
        project_id: row.get("project_id"),
        environment_id: row.get("environment_id"),
        hostname: row.get("hostname"),
        online: last_heartbeat.is_some_and(|seen| seen > Utc::now() - Duration::seconds(60)),
        last_heartbeat,
        snapshot,
        services,
        capabilities,
    })
}

fn command_from_row(row: sqlx::postgres::PgRow) -> Result<Command, StorageError> {
    command_from_row_ref(&row)
}

fn command_from_row_ref(row: &sqlx::postgres::PgRow) -> Result<Command, StorageError> {
    let status_text: String = row.get("status");
    let risk_text: String = row.get("risk_level");
    Ok(Command {
        id: row.get("id"),
        server_id: row.get("server_id"),
        command_type: serde_json::from_value(row.get("command_type"))?,
        created_at: row.get("created_at"),
        expires_at: row.get("expires_at"),
        status: parse_status(&status_text)?,
        idempotency_key: row.get("idempotency_key"),
        risk_level: parse_risk(&risk_text)?,
    })
}

fn parse_status(value: &str) -> Result<CommandStatus, StorageError> {
    match value {
        "QUEUED" => Ok(CommandStatus::QUEUED),
        "ACCEPTED" => Ok(CommandStatus::ACCEPTED),
        "RUNNING" => Ok(CommandStatus::RUNNING),
        "SUCCEEDED" => Ok(CommandStatus::SUCCEEDED),
        "FAILED" => Ok(CommandStatus::FAILED),
        "UNKNOWN" => Ok(CommandStatus::UNKNOWN),
        "EXPIRED" => Ok(CommandStatus::EXPIRED),
        _ => Err(StorageError::InvalidCommand),
    }
}

fn parse_risk(value: &str) -> Result<RiskLevel, StorageError> {
    match value {
        "LOW" => Ok(RiskLevel::LOW),
        "MEDIUM" => Ok(RiskLevel::MEDIUM),
        "HIGH" => Ok(RiskLevel::HIGH),
        "CRITICAL" => Ok(RiskLevel::CRITICAL),
        _ => Err(StorageError::InvalidCommand),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secret_hash_is_deterministic_without_storing_plaintext() {
        assert_eq!(hash_secret("abc"), hash_secret("abc"));
        assert_ne!(hash_secret("abc"), "abc");
    }

    #[test]
    fn terminal_status_parser_covers_unknown() {
        assert!(matches!(
            parse_status("UNKNOWN"),
            Ok(CommandStatus::UNKNOWN)
        ));
    }
}
