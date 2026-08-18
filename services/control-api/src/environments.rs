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
pub struct EnvironmentStore {
    pool: PgPool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProjectEnvironment {
    pub id: Uuid,
    pub project_id: Uuid,
    pub name: String,
    pub environment_type: String,
    pub description: String,
    pub is_protected: bool,
    pub sort_order: i32,
    pub server_count: i64,
    pub service_count: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateEnvironmentRequest {
    pub name: String,
    pub environment_type: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub is_protected: bool,
}

#[derive(Debug, Deserialize)]
pub struct UpdateEnvironmentRequest {
    pub name: String,
    pub environment_type: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub is_protected: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum EnvironmentError {
    #[error("environment not found")]
    NotFound,
    #[error("environment is protected")]
    Protected,
    #[error("environment is still in use")]
    InUse,
    #[error("invalid environment request")]
    Invalid,
    #[error(transparent)]
    Sql(#[from] sqlx::Error),
}

impl EnvironmentStore {
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
    ) -> Result<(), EnvironmentError> {
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
            Err(EnvironmentError::NotFound)
        }
    }

    pub async fn list(
        &self,
        organization_id: Uuid,
        project_id: Uuid,
    ) -> Result<Vec<ProjectEnvironment>, EnvironmentError> {
        self.authorize_project(organization_id, project_id).await?;
        let rows = sqlx::query(
            "SELECT e.id,e.project_id,e.name,e.type,e.description,e.is_protected,e.sort_order,e.created_at,e.updated_at,COUNT(DISTINCT s.id)::BIGINT AS server_count,COUNT(DISTINCT svc.id)::BIGINT AS service_count FROM environments e LEFT JOIN servers s ON s.environment_id=e.id AND s.organization_id=e.organization_id LEFT JOIN services svc ON svc.environment_id=e.id AND svc.organization_id=e.organization_id WHERE e.organization_id=$1 AND e.project_id=$2 GROUP BY e.id ORDER BY e.sort_order,e.created_at,e.name",
        )
        .bind(organization_id)
        .bind(project_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(environment_from_row).collect())
    }

    pub async fn create(
        &self,
        identity: crate::persistence::WebIdentity,
        project_id: Uuid,
        request: CreateEnvironmentRequest,
    ) -> Result<ProjectEnvironment, EnvironmentError> {
        self.authorize_project(identity.organization_id, project_id)
            .await?;
        let normalized = normalize(
            request.name,
            request.environment_type,
            request.description,
            request.is_protected,
        )?;
        self.ensure_name_available(identity.organization_id, project_id, &normalized.name, None)
            .await?;
        let id = Uuid::new_v4();
        let mut tx = self.pool.begin().await?;
        sqlx::query(
            "INSERT INTO environments(id,organization_id,project_id,name,type,description,is_protected,sort_order,created_at,updated_at) VALUES($1,$2,$3,$4,$5,$6,$7,$8,NOW(),NOW())",
        )
        .bind(id)
        .bind(identity.organization_id)
        .bind(project_id)
        .bind(&normalized.name)
        .bind(&normalized.environment_type)
        .bind(&normalized.description)
        .bind(normalized.is_protected)
        .bind(normalized.sort_order)
        .execute(&mut *tx)
        .await?;
        touch_project(&mut tx, identity.organization_id, project_id).await?;
        audit_event(
            &mut tx,
            identity,
            project_id,
            "environment.created",
            serde_json::json!({"environment_id":id,"name":normalized.name,"type":normalized.environment_type}),
        )
        .await?;
        tx.commit().await?;
        self.get(identity.organization_id, project_id, id).await
    }

    pub async fn update(
        &self,
        identity: crate::persistence::WebIdentity,
        project_id: Uuid,
        environment_id: Uuid,
        request: UpdateEnvironmentRequest,
    ) -> Result<ProjectEnvironment, EnvironmentError> {
        self.authorize_project(identity.organization_id, project_id)
            .await?;
        self.get(identity.organization_id, project_id, environment_id)
            .await?;
        let normalized = normalize(
            request.name,
            request.environment_type,
            request.description,
            request.is_protected,
        )?;
        self.ensure_name_available(
            identity.organization_id,
            project_id,
            &normalized.name,
            Some(environment_id),
        )
        .await?;
        let mut tx = self.pool.begin().await?;
        sqlx::query(
            "UPDATE environments SET name=$1,type=$2,description=$3,is_protected=$4,sort_order=$5,updated_at=NOW() WHERE id=$6 AND organization_id=$7 AND project_id=$8",
        )
        .bind(&normalized.name)
        .bind(&normalized.environment_type)
        .bind(&normalized.description)
        .bind(normalized.is_protected)
        .bind(normalized.sort_order)
        .bind(environment_id)
        .bind(identity.organization_id)
        .bind(project_id)
        .execute(&mut *tx)
        .await?;
        touch_project(&mut tx, identity.organization_id, project_id).await?;
        audit_event(
            &mut tx,
            identity,
            project_id,
            "environment.updated",
            serde_json::json!({"environment_id":environment_id,"name":normalized.name,"type":normalized.environment_type}),
        )
        .await?;
        tx.commit().await?;
        self.get(identity.organization_id, project_id, environment_id)
            .await
    }

    pub async fn delete(
        &self,
        identity: crate::persistence::WebIdentity,
        project_id: Uuid,
        environment_id: Uuid,
    ) -> Result<(), EnvironmentError> {
        let environment = self
            .get(identity.organization_id, project_id, environment_id)
            .await?;
        if environment.is_protected {
            return Err(EnvironmentError::Protected);
        }
        if environment.server_count > 0 || environment.service_count > 0 {
            return Err(EnvironmentError::InUse);
        }
        let mut tx = self.pool.begin().await?;
        let deleted = sqlx::query(
            "DELETE FROM environments WHERE id=$1 AND organization_id=$2 AND project_id=$3",
        )
        .bind(environment_id)
        .bind(identity.organization_id)
        .bind(project_id)
        .execute(&mut *tx)
        .await?;
        if deleted.rows_affected() != 1 {
            tx.rollback().await?;
            return Err(EnvironmentError::NotFound);
        }
        touch_project(&mut tx, identity.organization_id, project_id).await?;
        audit_event(
            &mut tx,
            identity,
            project_id,
            "environment.deleted",
            serde_json::json!({"environment_id":environment_id,"name":environment.name}),
        )
        .await?;
        tx.commit().await?;
        Ok(())
    }

    async fn get(
        &self,
        organization_id: Uuid,
        project_id: Uuid,
        environment_id: Uuid,
    ) -> Result<ProjectEnvironment, EnvironmentError> {
        let row = sqlx::query(
            "SELECT e.id,e.project_id,e.name,e.type,e.description,e.is_protected,e.sort_order,e.created_at,e.updated_at,(SELECT COUNT(*)::BIGINT FROM servers s WHERE s.environment_id=e.id AND s.organization_id=e.organization_id) AS server_count,(SELECT COUNT(*)::BIGINT FROM services svc WHERE svc.environment_id=e.id AND svc.organization_id=e.organization_id) AS service_count FROM environments e WHERE e.id=$1 AND e.organization_id=$2 AND e.project_id=$3",
        )
        .bind(environment_id)
        .bind(organization_id)
        .bind(project_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(EnvironmentError::NotFound)?;
        Ok(environment_from_row(row))
    }

    async fn ensure_name_available(
        &self,
        organization_id: Uuid,
        project_id: Uuid,
        name: &str,
        except_id: Option<Uuid>,
    ) -> Result<(), EnvironmentError> {
        let exists: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM environments WHERE organization_id=$1 AND project_id=$2 AND LOWER(name)=LOWER($3) AND ($4::uuid IS NULL OR id<>$4))",
        )
        .bind(organization_id)
        .bind(project_id)
        .bind(name)
        .bind(except_id)
        .fetch_one(&self.pool)
        .await?;
        if exists {
            Err(EnvironmentError::Invalid)
        } else {
            Ok(())
        }
    }
}

struct NormalizedEnvironment {
    name: String,
    environment_type: String,
    description: String,
    is_protected: bool,
    sort_order: i32,
}

fn normalize(
    name: String,
    environment_type: String,
    description: String,
    requested_protected: bool,
) -> Result<NormalizedEnvironment, EnvironmentError> {
    let name = name.trim();
    if name.is_empty() || name.len() > 120 {
        return Err(EnvironmentError::Invalid);
    }
    let description = description.trim();
    if description.len() > 4000 {
        return Err(EnvironmentError::Invalid);
    }
    let environment_type = environment_type.trim().to_lowercase();
    let sort_order = match environment_type.as_str() {
        "development" => 10,
        "preview" => 20,
        "staging" => 30,
        "production" => 40,
        "custom" => 100,
        _ => return Err(EnvironmentError::Invalid),
    };
    Ok(NormalizedEnvironment {
        name: name.to_string(),
        environment_type: environment_type.clone(),
        description: description.to_string(),
        is_protected: requested_protected || environment_type == "production",
        sort_order,
    })
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/projects/:project_id/environments",
            get(list_environments).post(create_environment),
        )
        .route(
            "/projects/:project_id/environments/:environment_id",
            get(get_environment)
                .put(update_environment)
                .delete(delete_environment),
        )
}

async fn list_environments(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(project_id): Path<Uuid>,
) -> Result<Json<Vec<ProjectEnvironment>>, ApiError> {
    let identity = web_identity(&state, &headers).await?;
    Ok(Json(
        state
            .environments
            .list(identity.organization_id, project_id)
            .await
            .map_err(map_environment)?,
    ))
}

async fn get_environment(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((project_id, environment_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<ProjectEnvironment>, ApiError> {
    let identity = web_identity(&state, &headers).await?;
    Ok(Json(
        state
            .environments
            .get(identity.organization_id, project_id, environment_id)
            .await
            .map_err(map_environment)?,
    ))
}

async fn create_environment(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(project_id): Path<Uuid>,
    Json(request): Json<CreateEnvironmentRequest>,
) -> Result<(StatusCode, Json<ProjectEnvironment>), ApiError> {
    let identity = web_identity(&state, &headers).await?;
    let environment = state
        .environments
        .create(identity, project_id, request)
        .await
        .map_err(map_environment)?;
    Ok((StatusCode::CREATED, Json(environment)))
}

async fn update_environment(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((project_id, environment_id)): Path<(Uuid, Uuid)>,
    Json(request): Json<UpdateEnvironmentRequest>,
) -> Result<Json<ProjectEnvironment>, ApiError> {
    let identity = web_identity(&state, &headers).await?;
    Ok(Json(
        state
            .environments
            .update(identity, project_id, environment_id, request)
            .await
            .map_err(map_environment)?,
    ))
}

async fn delete_environment(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((project_id, environment_id)): Path<(Uuid, Uuid)>,
) -> Result<StatusCode, ApiError> {
    let identity = web_identity(&state, &headers).await?;
    state
        .environments
        .delete(identity, project_id, environment_id)
        .await
        .map_err(map_environment)?;
    Ok(StatusCode::NO_CONTENT)
}

fn environment_from_row(row: sqlx::postgres::PgRow) -> ProjectEnvironment {
    ProjectEnvironment {
        id: row.get("id"),
        project_id: row.get("project_id"),
        name: row.get("name"),
        environment_type: row.get("type"),
        description: row.get("description"),
        is_protected: row.get("is_protected"),
        sort_order: row.get("sort_order"),
        server_count: row.get("server_count"),
        service_count: row.get("service_count"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }
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

fn map_environment(error: EnvironmentError) -> ApiError {
    match error {
        EnvironmentError::NotFound => api_error(
            StatusCode::NOT_FOUND,
            "ENVIRONMENT_NOT_FOUND",
            "environment not found",
        ),
        EnvironmentError::Protected => api_error(
            StatusCode::CONFLICT,
            "ENVIRONMENT_PROTECTED",
            "protected environments cannot be deleted",
        ),
        EnvironmentError::InUse => api_error(
            StatusCode::CONFLICT,
            "ENVIRONMENT_IN_USE",
            "environment is still referenced by a server or service",
        ),
        EnvironmentError::Invalid => api_error(
            StatusCode::BAD_REQUEST,
            "INVALID_REQUEST",
            "invalid environment request",
        ),
        other => {
            tracing::error!(error=%other, "environment storage error");
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
    fn production_is_always_protected() {
        let environment = normalize(
            "Production".into(),
            "production".into(),
            "".into(),
            false,
        )
        .unwrap();
        assert!(environment.is_protected);
        assert_eq!(environment.sort_order, 40);
    }

    #[test]
    fn custom_environment_can_be_unprotected() {
        let environment = normalize(
            "QA 2".into(),
            "custom".into(),
            "".into(),
            false,
        )
        .unwrap();
        assert!(!environment.is_protected);
        assert_eq!(environment.sort_order, 100);
    }
}
