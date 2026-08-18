use crate::{ApiError, AppState, api_error, web_identity};
use axum::{
    Json, Router,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    routing::get,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct ComposeStackStore {
    pool: PgPool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ComposeStack {
    pub id: Uuid,
    pub project_id: Uuid,
    pub environment_id: Uuid,
    pub environment_name: String,
    pub server_id: Uuid,
    pub server_hostname: String,
    pub name: String,
    pub compose_project_name: String,
    pub description: String,
    pub lifecycle_status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateComposeStackRequest {
    pub server_id: Uuid,
    pub name: String,
    pub compose_project_name: String,
    #[serde(default)]
    pub description: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdateComposeStackRequest {
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub lifecycle_status: String,
}

#[derive(Debug, thiserror::Error)]
pub enum ComposeStackError {
    #[error("project or stack not found")]
    NotFound,
    #[error("server does not belong to this project")]
    InvalidServer,
    #[error("stack already registered")]
    Conflict,
    #[error("invalid stack request")]
    Invalid,
    #[error(transparent)]
    Sql(#[from] sqlx::Error),
}

impl ComposeStackStore {
    pub async fn connect(database_url: &str) -> Result<Self, sqlx::Error> {
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(10)
            .connect(database_url)
            .await?;
        Ok(Self { pool })
    }

    async fn authorize_project(
        &self,
        organization_id: Uuid,
        project_id: Uuid,
    ) -> Result<(), ComposeStackError> {
        let exists: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM projects WHERE id=$1 AND organization_id=$2)",
        )
        .bind(project_id)
        .bind(organization_id)
        .fetch_one(&self.pool)
        .await?;
        if exists {
            Ok(())
        } else {
            Err(ComposeStackError::NotFound)
        }
    }

    async fn server_environment(
        &self,
        organization_id: Uuid,
        project_id: Uuid,
        server_id: Uuid,
    ) -> Result<Uuid, ComposeStackError> {
        sqlx::query_scalar(
            "SELECT environment_id FROM servers WHERE id=$1 AND organization_id=$2 AND project_id=$3",
        )
        .bind(server_id)
        .bind(organization_id)
        .bind(project_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(ComposeStackError::InvalidServer)
    }

    pub async fn list(
        &self,
        organization_id: Uuid,
        project_id: Uuid,
    ) -> Result<Vec<ComposeStack>, ComposeStackError> {
        self.authorize_project(organization_id, project_id).await?;
        let rows = sqlx::query(
            "SELECT cs.id,cs.project_id,cs.environment_id,e.name AS environment_name,cs.server_id,s.hostname AS server_hostname,cs.name,cs.compose_project_name,cs.description,cs.lifecycle_status,cs.created_at,cs.updated_at FROM compose_stacks cs JOIN environments e ON e.id=cs.environment_id AND e.organization_id=cs.organization_id JOIN servers s ON s.id=cs.server_id AND s.organization_id=cs.organization_id WHERE cs.organization_id=$1 AND cs.project_id=$2 ORDER BY cs.created_at,cs.name",
        )
        .bind(organization_id)
        .bind(project_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(stack_from_row).collect())
    }

    pub async fn create(
        &self,
        identity: crate::persistence::WebIdentity,
        project_id: Uuid,
        request: CreateComposeStackRequest,
    ) -> Result<ComposeStack, ComposeStackError> {
        self.authorize_project(identity.organization_id, project_id)
            .await?;
        let name = normalize_name(&request.name)?;
        let compose_project_name = normalize_compose_project_name(&request.compose_project_name)?;
        let description = normalize_description(&request.description)?;
        let environment_id = self
            .server_environment(identity.organization_id, project_id, request.server_id)
            .await?;

        let duplicate: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM compose_stacks WHERE organization_id=$1 AND server_id=$2 AND LOWER(compose_project_name)=LOWER($3))",
        )
        .bind(identity.organization_id)
        .bind(request.server_id)
        .bind(&compose_project_name)
        .fetch_one(&self.pool)
        .await?;
        if duplicate {
            return Err(ComposeStackError::Conflict);
        }

        let id = Uuid::new_v4();
        let mut tx = self.pool.begin().await?;
        sqlx::query(
            "INSERT INTO compose_stacks(id,organization_id,project_id,environment_id,server_id,name,compose_project_name,description,lifecycle_status,created_at,updated_at) VALUES($1,$2,$3,$4,$5,$6,$7,$8,'ACTIVE',NOW(),NOW())",
        )
        .bind(id)
        .bind(identity.organization_id)
        .bind(project_id)
        .bind(environment_id)
        .bind(request.server_id)
        .bind(&name)
        .bind(&compose_project_name)
        .bind(&description)
        .execute(&mut *tx)
        .await?;
        touch_project(&mut tx, identity.organization_id, project_id).await?;
        audit_event(
            &mut tx,
            identity,
            project_id,
            "stack.created",
            serde_json::json!({
                "stack_id": id,
                "server_id": request.server_id,
                "environment_id": environment_id,
                "compose_project_name": compose_project_name,
            }),
        )
        .await?;
        tx.commit().await?;
        self.get(identity.organization_id, project_id, id).await
    }

    pub async fn update(
        &self,
        identity: crate::persistence::WebIdentity,
        project_id: Uuid,
        stack_id: Uuid,
        request: UpdateComposeStackRequest,
    ) -> Result<ComposeStack, ComposeStackError> {
        self.get(identity.organization_id, project_id, stack_id)
            .await?;
        let name = normalize_name(&request.name)?;
        let description = normalize_description(&request.description)?;
        let lifecycle_status = normalize_lifecycle_status(&request.lifecycle_status)?;

        let mut tx = self.pool.begin().await?;
        let updated = sqlx::query(
            "UPDATE compose_stacks SET name=$1,description=$2,lifecycle_status=$3,updated_at=NOW() WHERE id=$4 AND organization_id=$5 AND project_id=$6",
        )
        .bind(&name)
        .bind(&description)
        .bind(&lifecycle_status)
        .bind(stack_id)
        .bind(identity.organization_id)
        .bind(project_id)
        .execute(&mut *tx)
        .await?;
        if updated.rows_affected() != 1 {
            tx.rollback().await?;
            return Err(ComposeStackError::NotFound);
        }
        touch_project(&mut tx, identity.organization_id, project_id).await?;
        audit_event(
            &mut tx,
            identity,
            project_id,
            "stack.updated",
            serde_json::json!({
                "stack_id": stack_id,
                "name": name,
                "lifecycle_status": lifecycle_status,
            }),
        )
        .await?;
        tx.commit().await?;
        self.get(identity.organization_id, project_id, stack_id)
            .await
    }

    pub async fn delete(
        &self,
        identity: crate::persistence::WebIdentity,
        project_id: Uuid,
        stack_id: Uuid,
    ) -> Result<(), ComposeStackError> {
        let stack = self
            .get(identity.organization_id, project_id, stack_id)
            .await?;
        let mut tx = self.pool.begin().await?;
        let deleted = sqlx::query(
            "DELETE FROM compose_stacks WHERE id=$1 AND organization_id=$2 AND project_id=$3",
        )
        .bind(stack_id)
        .bind(identity.organization_id)
        .bind(project_id)
        .execute(&mut *tx)
        .await?;
        if deleted.rows_affected() != 1 {
            tx.rollback().await?;
            return Err(ComposeStackError::NotFound);
        }
        touch_project(&mut tx, identity.organization_id, project_id).await?;
        audit_event(
            &mut tx,
            identity,
            project_id,
            "stack.deleted",
            serde_json::json!({
                "stack_id": stack_id,
                "server_id": stack.server_id,
                "compose_project_name": stack.compose_project_name,
            }),
        )
        .await?;
        tx.commit().await?;
        Ok(())
    }

    async fn get(
        &self,
        organization_id: Uuid,
        project_id: Uuid,
        stack_id: Uuid,
    ) -> Result<ComposeStack, ComposeStackError> {
        let row = sqlx::query(
            "SELECT cs.id,cs.project_id,cs.environment_id,e.name AS environment_name,cs.server_id,s.hostname AS server_hostname,cs.name,cs.compose_project_name,cs.description,cs.lifecycle_status,cs.created_at,cs.updated_at FROM compose_stacks cs JOIN environments e ON e.id=cs.environment_id AND e.organization_id=cs.organization_id JOIN servers s ON s.id=cs.server_id AND s.organization_id=cs.organization_id WHERE cs.id=$1 AND cs.organization_id=$2 AND cs.project_id=$3",
        )
        .bind(stack_id)
        .bind(organization_id)
        .bind(project_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(ComposeStackError::NotFound)?;
        Ok(stack_from_row(row))
    }
}

fn normalize_name(value: &str) -> Result<String, ComposeStackError> {
    let value = value.trim();
    if value.is_empty() || value.len() > 120 {
        Err(ComposeStackError::Invalid)
    } else {
        Ok(value.to_string())
    }
}

fn normalize_compose_project_name(value: &str) -> Result<String, ComposeStackError> {
    let value = value.trim();
    let valid = !value.is_empty()
        && value.len() <= 128
        && value
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
        && value
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '-' | '_'));
    if valid {
        Ok(value.to_string())
    } else {
        Err(ComposeStackError::Invalid)
    }
}

fn normalize_description(value: &str) -> Result<String, ComposeStackError> {
    let value = value.trim();
    if value.len() > 4000 {
        Err(ComposeStackError::Invalid)
    } else {
        Ok(value.to_string())
    }
}

fn normalize_lifecycle_status(value: &str) -> Result<String, ComposeStackError> {
    let value = value.trim().to_uppercase();
    if matches!(value.as_str(), "ACTIVE" | "PAUSED" | "ARCHIVED") {
        Ok(value)
    } else {
        Err(ComposeStackError::Invalid)
    }
}

fn stack_from_row(row: sqlx::postgres::PgRow) -> ComposeStack {
    ComposeStack {
        id: row.get("id"),
        project_id: row.get("project_id"),
        environment_id: row.get("environment_id"),
        environment_name: row.get("environment_name"),
        server_id: row.get("server_id"),
        server_hostname: row.get("server_hostname"),
        name: row.get("name"),
        compose_project_name: row.get("compose_project_name"),
        description: row.get("description"),
        lifecycle_status: row.get("lifecycle_status"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/projects/:project_id/stacks",
            get(list_stacks).post(create_stack),
        )
        .route(
            "/projects/:project_id/stacks/:stack_id",
            get(get_stack).put(update_stack).delete(delete_stack),
        )
}

async fn list_stacks(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(project_id): Path<Uuid>,
) -> Result<Json<Vec<ComposeStack>>, ApiError> {
    let identity = web_identity(&state, &headers).await?;
    Ok(Json(
        state
            .compose_stacks
            .list(identity.organization_id, project_id)
            .await
            .map_err(map_stack)?,
    ))
}

async fn get_stack(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((project_id, stack_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<ComposeStack>, ApiError> {
    let identity = web_identity(&state, &headers).await?;
    Ok(Json(
        state
            .compose_stacks
            .get(identity.organization_id, project_id, stack_id)
            .await
            .map_err(map_stack)?,
    ))
}

async fn create_stack(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(project_id): Path<Uuid>,
    Json(request): Json<CreateComposeStackRequest>,
) -> Result<(StatusCode, Json<ComposeStack>), ApiError> {
    let identity = web_identity(&state, &headers).await?;
    let stack = state
        .compose_stacks
        .create(identity, project_id, request)
        .await
        .map_err(map_stack)?;
    Ok((StatusCode::CREATED, Json(stack)))
}

async fn update_stack(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((project_id, stack_id)): Path<(Uuid, Uuid)>,
    Json(request): Json<UpdateComposeStackRequest>,
) -> Result<Json<ComposeStack>, ApiError> {
    let identity = web_identity(&state, &headers).await?;
    Ok(Json(
        state
            .compose_stacks
            .update(identity, project_id, stack_id, request)
            .await
            .map_err(map_stack)?,
    ))
}

async fn delete_stack(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((project_id, stack_id)): Path<(Uuid, Uuid)>,
) -> Result<StatusCode, ApiError> {
    let identity = web_identity(&state, &headers).await?;
    state
        .compose_stacks
        .delete(identity, project_id, stack_id)
        .await
        .map_err(map_stack)?;
    Ok(StatusCode::NO_CONTENT)
}

async fn touch_project(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    organization_id: Uuid,
    project_id: Uuid,
) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE projects SET updated_at=NOW() WHERE id=$1 AND organization_id=$2")
        .bind(project_id)
        .bind(organization_id)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

async fn audit_event(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    identity: crate::persistence::WebIdentity,
    project_id: Uuid,
    event_type: &str,
    data: serde_json::Value,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO audit_events(id,organization_id,actor,resource,action,request_id,result,source,timestamp) VALUES($1,$2,$3,$4,$5,$6,'SUCCEEDED','web',NOW())",
    )
    .bind(Uuid::new_v4())
    .bind(identity.organization_id)
    .bind(identity.user_id.to_string())
    .bind(project_id.to_string())
    .bind(event_type)
    .bind(Uuid::new_v4().to_string())
    .execute(&mut **tx)
    .await?;
    sqlx::query(
        "INSERT INTO domain_events(id,organization_id,event_type,resource_id,data,occurred_at) VALUES($1,$2,$3,$4,$5,NOW())",
    )
    .bind(Uuid::new_v4())
    .bind(identity.organization_id)
    .bind(event_type)
    .bind(project_id)
    .bind(data)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

fn map_stack(error: ComposeStackError) -> ApiError {
    match error {
        ComposeStackError::NotFound => api_error(
            StatusCode::NOT_FOUND,
            "STACK_NOT_FOUND",
            "stack or project not found",
        ),
        ComposeStackError::InvalidServer => api_error(
            StatusCode::BAD_REQUEST,
            "INVALID_SERVER",
            "server does not belong to this project",
        ),
        ComposeStackError::Conflict => api_error(
            StatusCode::CONFLICT,
            "STACK_ALREADY_REGISTERED",
            "this Compose project is already registered on the server",
        ),
        ComposeStackError::Invalid => api_error(
            StatusCode::BAD_REQUEST,
            "INVALID_REQUEST",
            "invalid stack request",
        ),
        other => {
            tracing::error!(error=%other, "compose stack storage error");
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
    fn compose_project_name_accepts_compose_safe_names() {
        assert_eq!(
            normalize_compose_project_name("my_stack-2").unwrap(),
            "my_stack-2"
        );
    }

    #[test]
    fn compose_project_name_rejects_uppercase_and_paths() {
        assert!(normalize_compose_project_name("MyStack").is_err());
        assert!(normalize_compose_project_name("../stack").is_err());
    }
}
