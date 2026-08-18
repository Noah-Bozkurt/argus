use crate::{ApiError, AppState, api_error, web_identity};
use axum::{
    Json, Router,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    routing::{get, post},
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};
use uuid::Uuid;

const EVENT_LOOKBACK_DAYS: i64 = 7;
const EVENT_SCAN_LIMIT: i64 = 5_000;
const INBOX_LIMIT: i64 = 200;

#[derive(Debug, Clone)]
pub struct NotificationStore {
    pool: PgPool,
}

#[derive(Debug, Clone, Serialize)]
pub struct NotificationRule {
    pub id: Uuid,
    pub project_id: Option<Uuid>,
    pub name: String,
    pub event_pattern: String,
    pub data_field: Option<String>,
    pub data_value: Option<String>,
    pub severity: String,
    pub enabled: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
pub struct NotificationItem {
    pub id: Uuid,
    pub project_id: Uuid,
    pub project_name: String,
    pub title: String,
    pub message: String,
    pub severity: String,
    pub source_event_type: String,
    pub source_occurred_at: DateTime<Utc>,
    pub read_at: Option<DateTime<Utc>>,
    pub acknowledged_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct NotificationInbox {
    pub unread_count: usize,
    pub unacknowledged_count: usize,
    pub notifications: Vec<NotificationItem>,
}

#[derive(Debug, Clone, Serialize)]
pub struct NotificationSyncResult {
    pub scanned_events: usize,
    pub enabled_rules: usize,
    pub created_notifications: u64,
    pub lookback_days: i64,
}

#[derive(Debug, Deserialize)]
pub struct CreateNotificationRuleRequest {
    pub project_id: Option<Uuid>,
    pub name: String,
    pub event_pattern: String,
    pub data_field: Option<String>,
    pub data_value: Option<String>,
    pub severity: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdateNotificationRuleRequest {
    pub project_id: Option<Uuid>,
    pub name: String,
    pub event_pattern: String,
    pub data_field: Option<String>,
    pub data_value: Option<String>,
    pub severity: String,
    pub enabled: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum NotificationError {
    #[error("notification resource not found")]
    NotFound,
    #[error("invalid notification request")]
    Invalid,
    #[error(transparent)]
    Sql(#[from] sqlx::Error),
}

#[derive(Debug)]
struct EventRow {
    id: Uuid,
    project_id: Uuid,
    project_name: String,
    event_type: String,
    data: serde_json::Value,
    occurred_at: DateTime<Utc>,
}

impl NotificationStore {
    pub async fn connect(database_url: &str) -> Result<Self, sqlx::Error> {
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(10)
            .connect(database_url)
            .await?;
        Ok(Self { pool })
    }

    pub async fn rules(
        &self,
        organization_id: Uuid,
    ) -> Result<Vec<NotificationRule>, NotificationError> {
        let rows = sqlx::query(
            "SELECT id,project_id,name,event_pattern,data_field,data_value,severity,enabled,created_at,updated_at FROM notification_rules WHERE organization_id=$1 ORDER BY enabled DESC,updated_at DESC,name",
        )
        .bind(organization_id)
        .fetch_all(&self.pool)
        .await?;
        rows.into_iter().map(rule_from_row).collect()
    }

    pub async fn create_rule(
        &self,
        identity: crate::persistence::WebIdentity,
        request: CreateNotificationRuleRequest,
    ) -> Result<NotificationRule, NotificationError> {
        validate_project_scope(&self.pool, identity.organization_id, request.project_id).await?;
        let normalized = normalize_rule(
            request.project_id,
            request.name,
            request.event_pattern,
            request.data_field,
            request.data_value,
            request.severity,
            true,
        )?;
        let id = Uuid::new_v4();
        let mut tx = self.pool.begin().await?;
        sqlx::query(
            "INSERT INTO notification_rules(id,organization_id,project_id,name,event_pattern,data_field,data_value,severity,enabled,created_by,created_at,updated_at) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,NOW(),NOW())",
        )
        .bind(id)
        .bind(identity.organization_id)
        .bind(normalized.project_id)
        .bind(&normalized.name)
        .bind(&normalized.event_pattern)
        .bind(&normalized.data_field)
        .bind(&normalized.data_value)
        .bind(&normalized.severity)
        .bind(normalized.enabled)
        .bind(identity.user_id)
        .execute(&mut *tx)
        .await?;
        audit_only(
            &mut tx,
            identity,
            request
                .project_id
                .map_or_else(|| "notifications".into(), |id| id.to_string()),
            "notification_rule.created",
        )
        .await?;
        tx.commit().await?;
        self.get_rule(identity.organization_id, id).await
    }

    pub async fn update_rule(
        &self,
        identity: crate::persistence::WebIdentity,
        rule_id: Uuid,
        request: UpdateNotificationRuleRequest,
    ) -> Result<NotificationRule, NotificationError> {
        self.get_rule(identity.organization_id, rule_id).await?;
        validate_project_scope(&self.pool, identity.organization_id, request.project_id).await?;
        let normalized = normalize_rule(
            request.project_id,
            request.name,
            request.event_pattern,
            request.data_field,
            request.data_value,
            request.severity,
            request.enabled,
        )?;
        let mut tx = self.pool.begin().await?;
        sqlx::query(
            "UPDATE notification_rules SET project_id=$1,name=$2,event_pattern=$3,data_field=$4,data_value=$5,severity=$6,enabled=$7,updated_at=NOW() WHERE id=$8 AND organization_id=$9",
        )
        .bind(normalized.project_id)
        .bind(&normalized.name)
        .bind(&normalized.event_pattern)
        .bind(&normalized.data_field)
        .bind(&normalized.data_value)
        .bind(&normalized.severity)
        .bind(normalized.enabled)
        .bind(rule_id)
        .bind(identity.organization_id)
        .execute(&mut *tx)
        .await?;
        audit_only(
            &mut tx,
            identity,
            normalized
                .project_id
                .map_or_else(|| "notifications".into(), |id| id.to_string()),
            "notification_rule.updated",
        )
        .await?;
        tx.commit().await?;
        self.get_rule(identity.organization_id, rule_id).await
    }

    pub async fn sync(
        &self,
        identity: crate::persistence::WebIdentity,
    ) -> Result<NotificationSyncResult, NotificationError> {
        let rules: Vec<NotificationRule> = self
            .rules(identity.organization_id)
            .await?
            .into_iter()
            .filter(|rule| rule.enabled)
            .collect();
        if rules.is_empty() {
            return Ok(NotificationSyncResult {
                scanned_events: 0,
                enabled_rules: 0,
                created_notifications: 0,
                lookback_days: EVENT_LOOKBACK_DAYS,
            });
        }
        let rows = sqlx::query(
            "SELECT e.id,p.id AS project_id,p.name AS project_name,e.event_type,e.data,e.occurred_at FROM domain_events e JOIN projects p ON p.id=e.resource_id AND p.organization_id=e.organization_id WHERE e.organization_id=$1 AND e.occurred_at >= NOW() - ($2::text || ' days')::interval ORDER BY e.occurred_at ASC LIMIT $3",
        )
        .bind(identity.organization_id)
        .bind(EVENT_LOOKBACK_DAYS)
        .bind(EVENT_SCAN_LIMIT)
        .fetch_all(&self.pool)
        .await?;
        let events: Vec<EventRow> = rows
            .into_iter()
            .map(|row| EventRow {
                id: row.get("id"),
                project_id: row.get("project_id"),
                project_name: row.get("project_name"),
                event_type: row.get("event_type"),
                data: row.get("data"),
                occurred_at: row.get("occurred_at"),
            })
            .collect();
        let mut created = 0_u64;
        let mut tx = self.pool.begin().await?;
        for event in &events {
            for rule in &rules {
                if rule
                    .project_id
                    .is_some_and(|project_id| project_id != event.project_id)
                    || !event_pattern_matches(&rule.event_pattern, &event.event_type)
                    || !data_filter_matches(
                        &event.data,
                        rule.data_field.as_deref(),
                        rule.data_value.as_deref(),
                    )
                {
                    continue;
                }
                let result = sqlx::query(
                    "INSERT INTO notifications(id,organization_id,project_id,rule_id,source_event_id,source_event_type,title,message,severity,source_occurred_at,created_at) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,NOW()) ON CONFLICT(rule_id,source_event_id) DO NOTHING",
                )
                .bind(Uuid::new_v4())
                .bind(identity.organization_id)
                .bind(event.project_id)
                .bind(rule.id)
                .bind(event.id)
                .bind(&event.event_type)
                .bind(&rule.name)
                .bind(event_summary(event))
                .bind(&rule.severity)
                .bind(event.occurred_at)
                .execute(&mut *tx)
                .await?;
                created += result.rows_affected();
            }
        }
        audit_only(
            &mut tx,
            identity,
            "notifications".into(),
            "notifications.synced",
        )
        .await?;
        tx.commit().await?;
        Ok(NotificationSyncResult {
            scanned_events: events.len(),
            enabled_rules: rules.len(),
            created_notifications: created,
            lookback_days: EVENT_LOOKBACK_DAYS,
        })
    }

    pub async fn inbox(
        &self,
        identity: crate::persistence::WebIdentity,
    ) -> Result<NotificationInbox, NotificationError> {
        let rows = sqlx::query(
            "SELECT n.id,n.project_id,p.name AS project_name,n.title,n.message,n.severity,n.source_event_type,n.source_occurred_at,s.read_at,s.acknowledged_at FROM notifications n JOIN projects p ON p.id=n.project_id LEFT JOIN notification_user_state s ON s.notification_id=n.id AND s.user_id=$2 WHERE n.organization_id=$1 ORDER BY n.source_occurred_at DESC,n.created_at DESC LIMIT $3",
        )
        .bind(identity.organization_id)
        .bind(identity.user_id)
        .bind(INBOX_LIMIT)
        .fetch_all(&self.pool)
        .await?;
        let notifications: Vec<NotificationItem> = rows
            .into_iter()
            .map(|row| NotificationItem {
                id: row.get("id"),
                project_id: row.get("project_id"),
                project_name: row.get("project_name"),
                title: row.get("title"),
                message: row.get("message"),
                severity: row.get("severity"),
                source_event_type: row.get("source_event_type"),
                source_occurred_at: row.get("source_occurred_at"),
                read_at: row.get("read_at"),
                acknowledged_at: row.get("acknowledged_at"),
            })
            .collect();
        Ok(NotificationInbox {
            unread_count: notifications
                .iter()
                .filter(|item| item.read_at.is_none())
                .count(),
            unacknowledged_count: notifications
                .iter()
                .filter(|item| item.acknowledged_at.is_none())
                .count(),
            notifications,
        })
    }

    pub async fn mark_read(
        &self,
        identity: crate::persistence::WebIdentity,
        notification_id: Uuid,
    ) -> Result<NotificationInbox, NotificationError> {
        self.ensure_notification(identity.organization_id, notification_id)
            .await?;
        sqlx::query(
            "INSERT INTO notification_user_state(notification_id,user_id,read_at,acknowledged_at) VALUES($1,$2,NOW(),NULL) ON CONFLICT(notification_id,user_id) DO UPDATE SET read_at=COALESCE(notification_user_state.read_at,NOW())",
        )
        .bind(notification_id)
        .bind(identity.user_id)
        .execute(&self.pool)
        .await?;
        self.inbox(identity).await
    }

    pub async fn acknowledge(
        &self,
        identity: crate::persistence::WebIdentity,
        notification_id: Uuid,
    ) -> Result<NotificationInbox, NotificationError> {
        self.ensure_notification(identity.organization_id, notification_id)
            .await?;
        sqlx::query(
            "INSERT INTO notification_user_state(notification_id,user_id,read_at,acknowledged_at) VALUES($1,$2,NOW(),NOW()) ON CONFLICT(notification_id,user_id) DO UPDATE SET read_at=COALESCE(notification_user_state.read_at,NOW()),acknowledged_at=COALESCE(notification_user_state.acknowledged_at,NOW())",
        )
        .bind(notification_id)
        .bind(identity.user_id)
        .execute(&self.pool)
        .await?;
        self.inbox(identity).await
    }

    async fn get_rule(
        &self,
        organization_id: Uuid,
        rule_id: Uuid,
    ) -> Result<NotificationRule, NotificationError> {
        let row = sqlx::query(
            "SELECT id,project_id,name,event_pattern,data_field,data_value,severity,enabled,created_at,updated_at FROM notification_rules WHERE id=$1 AND organization_id=$2",
        )
        .bind(rule_id)
        .bind(organization_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(NotificationError::NotFound)?;
        rule_from_row(row)
    }

    async fn ensure_notification(
        &self,
        organization_id: Uuid,
        notification_id: Uuid,
    ) -> Result<(), NotificationError> {
        let exists: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM notifications WHERE id=$1 AND organization_id=$2)",
        )
        .bind(notification_id)
        .bind(organization_id)
        .fetch_one(&self.pool)
        .await?;
        if exists {
            Ok(())
        } else {
            Err(NotificationError::NotFound)
        }
    }
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/notification-rules", get(list_rules).post(create_rule))
        .route(
            "/notification-rules/:rule_id",
            axum::routing::put(update_rule),
        )
        .route("/notifications", get(get_inbox))
        .route("/notifications/sync", post(sync_notifications))
        .route(
            "/notifications/:notification_id/read",
            post(mark_notification_read),
        )
        .route(
            "/notifications/:notification_id/acknowledge",
            post(acknowledge_notification),
        )
}

async fn list_rules(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<NotificationRule>>, ApiError> {
    let identity = web_identity(&state, &headers).await?;
    Ok(Json(
        state
            .notifications
            .rules(identity.organization_id)
            .await
            .map_err(map_notification)?,
    ))
}

async fn create_rule(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<CreateNotificationRuleRequest>,
) -> Result<(StatusCode, Json<NotificationRule>), ApiError> {
    let identity = web_identity(&state, &headers).await?;
    let rule = state
        .notifications
        .create_rule(identity, request)
        .await
        .map_err(map_notification)?;
    Ok((StatusCode::CREATED, Json(rule)))
}

async fn update_rule(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(rule_id): Path<Uuid>,
    Json(request): Json<UpdateNotificationRuleRequest>,
) -> Result<Json<NotificationRule>, ApiError> {
    let identity = web_identity(&state, &headers).await?;
    Ok(Json(
        state
            .notifications
            .update_rule(identity, rule_id, request)
            .await
            .map_err(map_notification)?,
    ))
}

async fn get_inbox(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<NotificationInbox>, ApiError> {
    let identity = web_identity(&state, &headers).await?;
    Ok(Json(
        state
            .notifications
            .inbox(identity)
            .await
            .map_err(map_notification)?,
    ))
}

async fn sync_notifications(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<NotificationSyncResult>, ApiError> {
    let identity = web_identity(&state, &headers).await?;
    Ok(Json(
        state
            .notifications
            .sync(identity)
            .await
            .map_err(map_notification)?,
    ))
}

async fn mark_notification_read(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(notification_id): Path<Uuid>,
) -> Result<Json<NotificationInbox>, ApiError> {
    let identity = web_identity(&state, &headers).await?;
    Ok(Json(
        state
            .notifications
            .mark_read(identity, notification_id)
            .await
            .map_err(map_notification)?,
    ))
}

async fn acknowledge_notification(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(notification_id): Path<Uuid>,
) -> Result<Json<NotificationInbox>, ApiError> {
    let identity = web_identity(&state, &headers).await?;
    Ok(Json(
        state
            .notifications
            .acknowledge(identity, notification_id)
            .await
            .map_err(map_notification)?,
    ))
}

fn normalize_rule(
    project_id: Option<Uuid>,
    name: String,
    event_pattern: String,
    data_field: Option<String>,
    data_value: Option<String>,
    severity: String,
    enabled: bool,
) -> Result<NotificationRule, NotificationError> {
    let name = required_text(&name, 1, 160)?;
    let event_pattern = normalize_event_pattern(&event_pattern)?;
    let data_field = normalize_data_field(data_field)?;
    let data_value = normalize_optional(data_value, 200)?;
    if data_field.is_some() != data_value.is_some() {
        return Err(NotificationError::Invalid);
    }
    let severity = severity.trim().to_uppercase();
    if !matches!(severity.as_str(), "INFO" | "WARNING" | "CRITICAL") {
        return Err(NotificationError::Invalid);
    }
    Ok(NotificationRule {
        id: Uuid::nil(),
        project_id,
        name,
        event_pattern,
        data_field,
        data_value,
        severity,
        enabled,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    })
}

async fn validate_project_scope(
    pool: &PgPool,
    organization_id: Uuid,
    project_id: Option<Uuid>,
) -> Result<(), NotificationError> {
    let Some(project_id) = project_id else {
        return Ok(());
    };
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM projects WHERE id=$1 AND organization_id=$2)",
    )
    .bind(project_id)
    .bind(organization_id)
    .fetch_one(pool)
    .await?;
    if exists {
        Ok(())
    } else {
        Err(NotificationError::Invalid)
    }
}

fn normalize_event_pattern(value: &str) -> Result<String, NotificationError> {
    let value = value.trim().to_ascii_lowercase();
    if value.is_empty()
        || value.len() > 120
        || value.starts_with('*')
        || value.matches('*').count() > 1
        || (value.contains('*') && !value.ends_with('*'))
        || !value.bytes().all(|byte| {
            byte.is_ascii_lowercase()
                || byte.is_ascii_digit()
                || matches!(byte, b'.' | b'_' | b'-' | b'*')
        })
    {
        return Err(NotificationError::Invalid);
    }
    Ok(value)
}

fn normalize_data_field(value: Option<String>) -> Result<Option<String>, NotificationError> {
    let Some(value) = normalize_optional(value, 120)? else {
        return Ok(None);
    };
    if value.split('.').count() > 4
        || value.split('.').any(|part| {
            part.is_empty()
                || !part
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
        })
    {
        return Err(NotificationError::Invalid);
    }
    Ok(Some(value))
}

fn event_pattern_matches(pattern: &str, event_type: &str) -> bool {
    match pattern.strip_suffix('*') {
        Some(prefix) => event_type.starts_with(prefix),
        None => event_type == pattern,
    }
}

fn data_filter_matches(
    data: &serde_json::Value,
    field: Option<&str>,
    expected: Option<&str>,
) -> bool {
    let (Some(field), Some(expected)) = (field, expected) else {
        return true;
    };
    let mut current = data;
    for part in field.split('.') {
        let Some(next) = current.get(part) else {
            return false;
        };
        current = next;
    }
    let actual = match current {
        serde_json::Value::String(value) => value.clone(),
        serde_json::Value::Bool(value) => value.to_string(),
        serde_json::Value::Number(value) => value.to_string(),
        _ => return false,
    };
    actual.trim().eq_ignore_ascii_case(expected.trim())
}

fn event_summary(event: &EventRow) -> String {
    let detail = ["name", "hostname", "version", "status", "to"]
        .iter()
        .find_map(|key| event.data.get(key).and_then(serde_json::Value::as_str));
    match detail {
        Some(detail) => format!(
            "{} in {} — {}",
            event.event_type, event.project_name, detail
        ),
        None => format!("{} in {}", event.event_type, event.project_name),
    }
}

fn rule_from_row(row: sqlx::postgres::PgRow) -> Result<NotificationRule, NotificationError> {
    Ok(NotificationRule {
        id: row.get("id"),
        project_id: row.get("project_id"),
        name: row.get("name"),
        event_pattern: row.get("event_pattern"),
        data_field: row.get("data_field"),
        data_value: row.get("data_value"),
        severity: row.get("severity"),
        enabled: row.get("enabled"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}

fn required_text(value: &str, min: usize, max: usize) -> Result<String, NotificationError> {
    let value = value.trim();
    if value.len() < min || value.len() > max {
        Err(NotificationError::Invalid)
    } else {
        Ok(value.to_string())
    }
}

fn normalize_optional(
    value: Option<String>,
    max: usize,
) -> Result<Option<String>, NotificationError> {
    match value.map(|value| value.trim().to_string()) {
        Some(value) if value.is_empty() => Ok(None),
        Some(value) if value.len() <= max => Ok(Some(value)),
        Some(_) => Err(NotificationError::Invalid),
        None => Ok(None),
    }
}

async fn audit_only(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    identity: crate::persistence::WebIdentity,
    resource: String,
    action: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO audit_events(id,organization_id,actor,resource,action,request_id,result,source,timestamp) VALUES($1,$2,$3,$4,$5,$6,'SUCCEEDED','web',NOW())",
    )
    .bind(Uuid::new_v4())
    .bind(identity.organization_id)
    .bind(identity.user_id.to_string())
    .bind(resource)
    .bind(action)
    .bind(Uuid::new_v4().to_string())
    .execute(&mut **tx)
    .await?;
    Ok(())
}

fn map_notification(error: NotificationError) -> ApiError {
    match error {
        NotificationError::NotFound => api_error(
            StatusCode::NOT_FOUND,
            "NOTIFICATION_NOT_FOUND",
            "notification resource not found",
        ),
        NotificationError::Invalid => api_error(
            StatusCode::BAD_REQUEST,
            "INVALID_REQUEST",
            "invalid notification request",
        ),
        other => {
            tracing::error!(error=%other, "notification storage error");
            api_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "INTERNAL_ERROR",
                "internal error",
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_patterns_are_exact_or_suffix_wildcards() {
        assert!(normalize_event_pattern("incident.*").is_ok());
        assert!(event_pattern_matches("incident.*", "incident.created"));
        assert!(!event_pattern_matches("incident.*", "deployment.failed"));
        assert!(normalize_event_pattern("*.failed").is_err());
        assert!(normalize_event_pattern("incident.*.failed").is_err());
    }

    #[test]
    fn data_filter_matches_scalar_nested_values() {
        let data = serde_json::json!({"status":"DOWN","nested":{"value":42}});
        assert!(data_filter_matches(&data, Some("status"), Some("down")));
        assert!(data_filter_matches(&data, Some("nested.value"), Some("42")));
        assert!(!data_filter_matches(&data, Some("missing"), Some("x")));
    }
}
