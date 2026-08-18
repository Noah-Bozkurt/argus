use chrono::{DateTime, Duration, Utc};
use serde::Serialize;
use sqlx::{PgPool, Row};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct MaintenanceStore { pool: PgPool }

#[derive(Debug, Clone, Serialize)]
pub struct MaintenanceWindow {
    pub id: Uuid,
    pub server_id: Uuid,
    pub starts_at: DateTime<Utc>,
    pub ends_at: DateTime<Utc>,
    pub reason: String,
    pub created_by: Uuid,
    pub created_at: DateTime<Utc>,
    pub ended_at: Option<DateTime<Utc>>,
}

impl MaintenanceStore {
    pub async fn connect(database_url: &str) -> Result<Self, sqlx::Error> {
        let pool = sqlx::postgres::PgPoolOptions::new().max_connections(5).connect(database_url).await?;
        Ok(Self { pool })
    }

    async fn assert_server(&self, organization_id: Uuid, server_id: Uuid) -> Result<(), sqlx::Error> {
        let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM servers WHERE id=$1 AND organization_id=$2)")
            .bind(server_id).bind(organization_id).fetch_one(&self.pool).await?;
        if exists { Ok(()) } else { Err(sqlx::Error::RowNotFound) }
    }

    pub async fn start(&self, organization_id: Uuid, user_id: Uuid, server_id: Uuid, duration_minutes: i64, reason: &str) -> Result<MaintenanceWindow, sqlx::Error> {
        self.assert_server(organization_id, server_id).await?;
        let now = Utc::now();
        let ends_at = now + Duration::minutes(duration_minutes);
        let id = Uuid::new_v4();
        let row = sqlx::query("INSERT INTO maintenance_windows(id,organization_id,server_id,starts_at,ends_at,reason,created_by) VALUES($1,$2,$3,$4,$5,$6,$7) RETURNING id,server_id,starts_at,ends_at,reason,created_by,created_at,ended_at")
            .bind(id).bind(organization_id).bind(server_id).bind(now).bind(ends_at).bind(reason).bind(user_id).fetch_one(&self.pool).await?;
        Ok(from_row(row))
    }

    pub async fn end_active(&self, organization_id: Uuid, server_id: Uuid) -> Result<u64, sqlx::Error> {
        self.assert_server(organization_id, server_id).await?;
        Ok(sqlx::query("UPDATE maintenance_windows SET ended_at=NOW() WHERE organization_id=$1 AND server_id=$2 AND ended_at IS NULL AND starts_at <= NOW() AND ends_at > NOW()")
            .bind(organization_id).bind(server_id).execute(&self.pool).await?.rows_affected())
    }

    pub async fn active(&self, organization_id: Uuid, server_id: Uuid) -> Result<Option<MaintenanceWindow>, sqlx::Error> {
        self.assert_server(organization_id, server_id).await?;
        let row = sqlx::query("SELECT id,server_id,starts_at,ends_at,reason,created_by,created_at,ended_at FROM maintenance_windows WHERE organization_id=$1 AND server_id=$2 AND ended_at IS NULL AND starts_at <= NOW() AND ends_at > NOW() ORDER BY ends_at DESC LIMIT 1")
            .bind(organization_id).bind(server_id).fetch_optional(&self.pool).await?;
        Ok(row.map(from_row))
    }

    pub async fn history(&self, organization_id: Uuid, server_id: Uuid) -> Result<Vec<MaintenanceWindow>, sqlx::Error> {
        self.assert_server(organization_id, server_id).await?;
        let rows = sqlx::query("SELECT id,server_id,starts_at,ends_at,reason,created_by,created_at,ended_at FROM maintenance_windows WHERE organization_id=$1 AND server_id=$2 ORDER BY starts_at DESC LIMIT 50")
            .bind(organization_id).bind(server_id).fetch_all(&self.pool).await?;
        Ok(rows.into_iter().map(from_row).collect())
    }
}

fn from_row(row: sqlx::postgres::PgRow) -> MaintenanceWindow {
    MaintenanceWindow { id: row.get("id"), server_id: row.get("server_id"), starts_at: row.get("starts_at"), ends_at: row.get("ends_at"), reason: row.get("reason"), created_by: row.get("created_by"), created_at: row.get("created_at"), ended_at: row.get("ended_at") }
}
