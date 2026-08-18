use crate::persistence::WebIdentity;
use protocol::SystemSnapshot;
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct DesiredStateStore {
    pool: PgPool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DesiredState {
    pub mode: String,
    pub firewall_enabled: Option<bool>,
    pub ssh_password_auth: Option<bool>,
    pub ssh_root_login: Option<String>,
    pub automatic_security_updates: Option<bool>,
}

impl Default for DesiredState {
    fn default() -> Self {
        Self {
            mode: "MONITOR".into(),
            firewall_enabled: None,
            ssh_password_auth: None,
            ssh_root_login: None,
            automatic_security_updates: None,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct DriftItem {
    pub field: String,
    pub desired: String,
    pub actual: String,
    pub severity: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct DesiredStateView {
    pub policy: DesiredState,
    pub drift: Vec<DriftItem>,
    pub enforcement_available: bool,
    pub firewall_enforcement_available: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum DesiredStateError {
    #[error("permission denied")]
    PermissionDenied,
    #[error("enforcement is not available for security/network policy yet")]
    EnforcementUnavailable,
    #[error("invalid desired state")]
    Invalid,
    #[error(transparent)]
    Sql(#[from] sqlx::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

impl DesiredStateStore {
    pub async fn connect(database_url: &str) -> Result<Self, sqlx::Error> {
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(10)
            .connect(database_url)
            .await?;
        Ok(Self { pool })
    }

    async fn authorize(
        &self,
        identity: WebIdentity,
        server_id: Uuid,
    ) -> Result<(), DesiredStateError> {
        let allowed: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM servers WHERE id=$1 AND organization_id=$2)",
        )
        .bind(server_id)
        .bind(identity.organization_id)
        .fetch_one(&self.pool)
        .await?;
        if allowed {
            Ok(())
        } else {
            Err(DesiredStateError::PermissionDenied)
        }
    }

    pub async fn get(
        &self,
        identity: WebIdentity,
        server_id: Uuid,
    ) -> Result<DesiredStateView, DesiredStateError> {
        self.authorize(identity, server_id).await?;
        let policy = if let Some(row) = sqlx::query(
            "SELECT mode,firewall_enabled,ssh_password_auth,ssh_root_login,automatic_security_updates FROM server_policies WHERE server_id=$1 AND organization_id=$2",
        )
        .bind(server_id)
        .bind(identity.organization_id)
        .fetch_optional(&self.pool)
        .await?
        {
            DesiredState {
                mode: row.get("mode"),
                firewall_enabled: row.get("firewall_enabled"),
                ssh_password_auth: row.get("ssh_password_auth"),
                ssh_root_login: row.get("ssh_root_login"),
                automatic_security_updates: row.get("automatic_security_updates"),
            }
        } else {
            DesiredState::default()
        };

        let snapshot: Option<SystemSnapshot> = sqlx::query_scalar::<_, Option<serde_json::Value>>(
            "SELECT snapshot FROM agents WHERE server_id=$1",
        )
        .bind(server_id)
        .fetch_optional(&self.pool)
        .await?
        .flatten()
        .map(serde_json::from_value)
        .transpose()?;
        let drift = snapshot
            .as_ref()
            .map(|snapshot| calculate_drift(&policy, snapshot))
            .unwrap_or_default();
        let firewall_enforcement_available = policy.firewall_enabled == Some(true)
            && snapshot
                .as_ref()
                .is_some_and(|snapshot| snapshot.security.available);
        Ok(DesiredStateView {
            policy,
            drift,
            enforcement_available: false,
            firewall_enforcement_available,
        })
    }

    pub async fn allows_firewall_enable(
        &self,
        identity: WebIdentity,
        server_id: Uuid,
    ) -> Result<bool, DesiredStateError> {
        self.authorize(identity, server_id).await?;
        let desired: Option<bool> = sqlx::query_scalar(
            "SELECT firewall_enabled FROM server_policies WHERE server_id=$1 AND organization_id=$2",
        )
        .bind(server_id)
        .bind(identity.organization_id)
        .fetch_optional(&self.pool)
        .await?
        .flatten();
        Ok(desired == Some(true))
    }

    pub async fn update(
        &self,
        identity: WebIdentity,
        server_id: Uuid,
        policy: DesiredState,
    ) -> Result<DesiredStateView, DesiredStateError> {
        self.authorize(identity, server_id).await?;
        if policy.mode != "MONITOR" && policy.mode != "ENFORCE" {
            return Err(DesiredStateError::Invalid);
        }
        if policy.mode == "ENFORCE" {
            return Err(DesiredStateError::EnforcementUnavailable);
        }
        if policy
            .ssh_root_login
            .as_deref()
            .is_some_and(|value| !matches!(value, "no" | "prohibit-password" | "yes"))
        {
            return Err(DesiredStateError::Invalid);
        }

        let mut tx = self.pool.begin().await?;
        sqlx::query(
            "INSERT INTO server_policies(server_id,organization_id,mode,firewall_enabled,ssh_password_auth,ssh_root_login,automatic_security_updates,updated_by,updated_at) VALUES($1,$2,$3,$4,$5,$6,$7,$8,NOW()) ON CONFLICT(server_id) DO UPDATE SET mode=EXCLUDED.mode,firewall_enabled=EXCLUDED.firewall_enabled,ssh_password_auth=EXCLUDED.ssh_password_auth,ssh_root_login=EXCLUDED.ssh_root_login,automatic_security_updates=EXCLUDED.automatic_security_updates,updated_by=EXCLUDED.updated_by,updated_at=NOW()",
        )
        .bind(server_id)
        .bind(identity.organization_id)
        .bind(&policy.mode)
        .bind(policy.firewall_enabled)
        .bind(policy.ssh_password_auth)
        .bind(&policy.ssh_root_login)
        .bind(policy.automatic_security_updates)
        .bind(identity.user_id)
        .execute(&mut *tx)
        .await?;

        sqlx::query(
            "INSERT INTO audit_events(id,organization_id,actor,resource,action,request_id,result,source,timestamp) VALUES($1,$2,$3,$4,$5,$6,$7,$8,NOW())",
        )
        .bind(Uuid::new_v4())
        .bind(identity.organization_id)
        .bind(identity.user_id.to_string())
        .bind(server_id.to_string())
        .bind("server.desired_state.updated")
        .bind(Uuid::new_v4().to_string())
        .bind("SUCCEEDED")
        .bind("web")
        .execute(&mut *tx)
        .await?;

        sqlx::query(
            "INSERT INTO domain_events(id,organization_id,event_type,resource_id,data,occurred_at) VALUES($1,$2,$3,$4,$5,NOW())",
        )
        .bind(Uuid::new_v4())
        .bind(identity.organization_id)
        .bind("server.desired_state.updated")
        .bind(server_id)
        .bind(serde_json::json!({"mode": policy.mode}))
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;

        self.get(identity, server_id).await
    }
}

fn calculate_drift(policy: &DesiredState, snapshot: &SystemSnapshot) -> Vec<DriftItem> {
    let mut drift = Vec::new();
    if let Some(desired) = policy.firewall_enabled {
        let actual = snapshot.security.firewall_status == "active";
        if desired != actual {
            drift.push(item("firewall_enabled", desired, actual, "HIGH"));
        }
    }
    if let Some(desired) = policy.ssh_password_auth {
        if snapshot.security.ssh_password_auth != Some(desired) {
            drift.push(DriftItem {
                field: "ssh_password_auth".into(),
                desired: desired.to_string(),
                actual: snapshot
                    .security
                    .ssh_password_auth
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| "unknown".into()),
                severity: "HIGH".into(),
            });
        }
    }
    if let Some(desired) = &policy.ssh_root_login {
        if desired != &snapshot.security.ssh_root_login {
            drift.push(DriftItem {
                field: "ssh_root_login".into(),
                desired: desired.clone(),
                actual: snapshot.security.ssh_root_login.clone(),
                severity: "HIGH".into(),
            });
        }
    }
    if let Some(desired) = policy.automatic_security_updates {
        let actual = snapshot.security.automatic_security_updates;
        if desired != actual {
            drift.push(item(
                "automatic_security_updates",
                desired,
                actual,
                "MEDIUM",
            ));
        }
    }
    drift
}

fn item(field: &str, desired: bool, actual: bool, severity: &str) -> DriftItem {
    DriftItem {
        field: field.into(),
        desired: desired.to_string(),
        actual: actual.to_string(),
        severity: severity.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use protocol::{BackupState, DiagnosticsState, DockerState, SecurityState, UpdateState};

    fn snapshot() -> SystemSnapshot {
        SystemSnapshot {
            server_id: Uuid::new_v4(),
            hostname: "test".into(),
            os: "Ubuntu".into(),
            kernel: "test".into(),
            architecture: "x86_64".into(),
            cpu_percent: 0.0,
            ram_percent: 0.0,
            disk_percent: 0.0,
            load: 0.0,
            uptime_seconds: 1,
            agent_version: "test".into(),
            updates: UpdateState::default(),
            diagnostics: DiagnosticsState::default(),
            docker: DockerState::default(),
            security: SecurityState {
                available: true,
                firewall_status: "inactive".into(),
                firewall_rules: vec![],
                ssh_password_auth: Some(true),
                ssh_root_login: "yes".into(),
                automatic_security_updates: false,
                findings: vec![],
            },
            backups: BackupState::default(),
            captured_at: Utc::now(),
        }
    }

    #[test]
    fn detects_security_drift() {
        let policy = DesiredState {
            mode: "MONITOR".into(),
            firewall_enabled: Some(true),
            ssh_password_auth: Some(false),
            ssh_root_login: Some("no".into()),
            automatic_security_updates: Some(true),
        };
        assert_eq!(calculate_drift(&policy, &snapshot()).len(), 4);
    }

    #[test]
    fn ignores_unconfigured_policy_fields() {
        assert!(calculate_drift(&DesiredState::default(), &snapshot()).is_empty());
    }
}
