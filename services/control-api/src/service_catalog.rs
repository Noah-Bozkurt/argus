use crate::{ApiError, AppState, api_error, web_identity};
use axum::{
    Json, Router,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    routing::get,
};
use chrono::{DateTime, Utc};
use reqwest::Url;
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct ServiceCatalogStore {
    pool: PgPool,
}

#[derive(Debug, Clone, Serialize)]
pub struct CatalogService {
    pub id: Uuid,
    pub project_id: Uuid,
    pub environment_id: Option<Uuid>,
    pub server_id: Option<Uuid>,
    pub repository_id: Option<Uuid>,
    pub name: String,
    pub description: String,
    pub service_type: String,
    pub runtime: Option<String>,
    pub owner_user_id: Option<Uuid>,
    pub endpoint_url: Option<String>,
    pub lifecycle_status: String,
    pub health_status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateServiceRequest {
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub service_type: String,
    pub runtime: Option<String>,
    pub repository_id: Option<Uuid>,
    pub environment_id: Option<Uuid>,
    pub server_id: Option<Uuid>,
    pub owner_user_id: Option<Uuid>,
    pub endpoint_url: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateServiceRequest {
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub service_type: String,
    pub runtime: Option<String>,
    pub repository_id: Option<Uuid>,
    pub environment_id: Option<Uuid>,
    pub server_id: Option<Uuid>,
    pub owner_user_id: Option<Uuid>,
    pub endpoint_url: Option<String>,
    pub lifecycle_status: String,
}

#[derive(Debug, thiserror::Error)]
pub enum ServiceCatalogError {
    #[error("service not found")]
    NotFound,
    #[error("invalid service catalog request")]
    Invalid,
    #[error(transparent)]
    Sql(#[from] sqlx::Error),
}

impl ServiceCatalogStore {
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
    ) -> Result<(), ServiceCatalogError> {
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
            Err(ServiceCatalogError::NotFound)
        }
    }

    pub async fn list(
        &self,
        organization_id: Uuid,
        project_id: Uuid,
    ) -> Result<Vec<CatalogService>, ServiceCatalogError> {
        self.authorize_project(organization_id, project_id).await?;
        let rows = sqlx::query(
            "SELECT id,project_id,environment_id,server_id,repository_id,name,description,service_type,runtime,owner_user_id,endpoint_url,lifecycle_status,status,created_at,updated_at FROM services WHERE organization_id=$1 AND project_id=$2 ORDER BY CASE lifecycle_status WHEN 'ACTIVE' THEN 0 WHEN 'PAUSED' THEN 1 ELSE 2 END,updated_at DESC,name",
        )
        .bind(organization_id)
        .bind(project_id)
        .fetch_all(&self.pool)
        .await?;
        rows.into_iter().map(service_from_row).collect()
    }

    pub async fn create(
        &self,
        identity: crate::persistence::WebIdentity,
        project_id: Uuid,
        request: CreateServiceRequest,
    ) -> Result<CatalogService, ServiceCatalogError> {
        self.authorize_project(identity.organization_id, project_id)
            .await?;
        let normalized = self
            .normalize(
                identity.organization_id,
                project_id,
                request.name,
                request.description,
                request.service_type,
                request.runtime,
                request.repository_id,
                request.environment_id,
                request.server_id,
                request.owner_user_id,
                request.endpoint_url,
                "ACTIVE".into(),
            )
            .await?;
        let id = Uuid::new_v4();
        let mut tx = self.pool.begin().await?;
        sqlx::query(
            "INSERT INTO services(id,organization_id,project_id,environment_id,server_id,repository_id,name,description,service_type,runtime,owner_user_id,endpoint_url,lifecycle_status,status,created_at,updated_at) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,'UNKNOWN',NOW(),NOW())",
        )
        .bind(id)
        .bind(identity.organization_id)
        .bind(project_id)
        .bind(normalized.environment_id)
        .bind(normalized.server_id)
        .bind(normalized.repository_id)
        .bind(&normalized.name)
        .bind(&normalized.description)
        .bind(&normalized.service_type)
        .bind(&normalized.runtime)
        .bind(normalized.owner_user_id)
        .bind(&normalized.endpoint_url)
        .bind(&normalized.lifecycle_status)
        .execute(&mut *tx)
        .await?;
        touch_project(&mut tx, identity.organization_id, project_id).await?;
        audit_event(
            &mut tx,
            identity,
            project_id,
            "service.created",
            serde_json::json!({"service_id":id,"name":normalized.name,"type":normalized.service_type}),
        )
        .await?;
        tx.commit().await?;
        self.get(identity.organization_id, project_id, id).await
    }

    pub async fn update(
        &self,
        identity: crate::persistence::WebIdentity,
        project_id: Uuid,
        service_id: Uuid,
        request: UpdateServiceRequest,
    ) -> Result<CatalogService, ServiceCatalogError> {
        self.authorize_project(identity.organization_id, project_id)
            .await?;
        self.get(identity.organization_id, project_id, service_id)
            .await?;
        let normalized = self
            .normalize(
                identity.organization_id,
                project_id,
                request.name,
                request.description,
                request.service_type,
                request.runtime,
                request.repository_id,
                request.environment_id,
                request.server_id,
                request.owner_user_id,
                request.endpoint_url,
                request.lifecycle_status,
            )
            .await?;
        let mut tx = self.pool.begin().await?;
        sqlx::query(
            "UPDATE services SET environment_id=$1,server_id=$2,repository_id=$3,name=$4,description=$5,service_type=$6,runtime=$7,owner_user_id=$8,endpoint_url=$9,lifecycle_status=$10,updated_at=NOW() WHERE id=$11 AND organization_id=$12 AND project_id=$13",
        )
        .bind(normalized.environment_id)
        .bind(normalized.server_id)
        .bind(normalized.repository_id)
        .bind(&normalized.name)
        .bind(&normalized.description)
        .bind(&normalized.service_type)
        .bind(&normalized.runtime)
        .bind(normalized.owner_user_id)
        .bind(&normalized.endpoint_url)
        .bind(&normalized.lifecycle_status)
        .bind(service_id)
        .bind(identity.organization_id)
        .bind(project_id)
        .execute(&mut *tx)
        .await?;
        touch_project(&mut tx, identity.organization_id, project_id).await?;
        audit_event(
            &mut tx,
            identity,
            project_id,
            "service.updated",
            serde_json::json!({"service_id":service_id,"name":normalized.name,"lifecycle_status":normalized.lifecycle_status}),
        )
        .await?;
        tx.commit().await?;
        self.get(identity.organization_id, project_id, service_id)
            .await
    }

    pub async fn delete(
        &self,
        identity: crate::persistence::WebIdentity,
        project_id: Uuid,
        service_id: Uuid,
    ) -> Result<(), ServiceCatalogError> {
        self.authorize_project(identity.organization_id, project_id)
            .await?;
        let service = self
            .get(identity.organization_id, project_id, service_id)
            .await?;
        let mut tx = self.pool.begin().await?;
        let deleted = sqlx::query(
            "DELETE FROM services WHERE id=$1 AND organization_id=$2 AND project_id=$3",
        )
        .bind(service_id)
        .bind(identity.organization_id)
        .bind(project_id)
        .execute(&mut *tx)
        .await?;
        if deleted.rows_affected() != 1 {
            tx.rollback().await?;
            return Err(ServiceCatalogError::NotFound);
        }
        touch_project(&mut tx, identity.organization_id, project_id).await?;
        audit_event(
            &mut tx,
            identity,
            project_id,
            "service.deleted",
            serde_json::json!({"service_id":service_id,"name":service.name}),
        )
        .await?;
        tx.commit().await?;
        Ok(())
    }

    async fn get(
        &self,
        organization_id: Uuid,
        project_id: Uuid,
        service_id: Uuid,
    ) -> Result<CatalogService, ServiceCatalogError> {
        let row = sqlx::query(
            "SELECT id,project_id,environment_id,server_id,repository_id,name,description,service_type,runtime,owner_user_id,endpoint_url,lifecycle_status,status,created_at,updated_at FROM services WHERE id=$1 AND organization_id=$2 AND project_id=$3",
        )
        .bind(service_id)
        .bind(organization_id)
        .bind(project_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(ServiceCatalogError::NotFound)?;
        service_from_row(row)
    }

    #[allow(clippy::too_many_arguments)]
    async fn normalize(
        &self,
        organization_id: Uuid,
        project_id: Uuid,
        name: String,
        description: String,
        service_type: String,
        runtime: Option<String>,
        repository_id: Option<Uuid>,
        mut environment_id: Option<Uuid>,
        server_id: Option<Uuid>,
        owner_user_id: Option<Uuid>,
        endpoint_url: Option<String>,
        lifecycle_status: String,
    ) -> Result<NormalizedService, ServiceCatalogError> {
        let name = required_text(&name, 1, 160)?;
        let description = optional_text(&description, 8000)?;
        let service_type = service_type.trim().to_lowercase();
        if !matches!(
            service_type.as_str(),
            "web" | "api" | "worker" | "database" | "queue" | "cron" | "other"
        ) {
            return Err(ServiceCatalogError::Invalid);
        }
        let runtime = optional_value(runtime, 120)?;
        let endpoint_url = normalize_endpoint(endpoint_url)?;
        let lifecycle_status = lifecycle_status.trim().to_uppercase();
        if !matches!(lifecycle_status.as_str(), "ACTIVE" | "PAUSED" | "ARCHIVED") {
            return Err(ServiceCatalogError::Invalid);
        }

        if let Some(repository_id) = repository_id {
            let valid: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM project_repositories WHERE id=$1 AND organization_id=$2 AND project_id=$3)",
            )
            .bind(repository_id)
            .bind(organization_id)
            .bind(project_id)
            .fetch_one(&self.pool)
            .await?;
            if !valid {
                return Err(ServiceCatalogError::Invalid);
            }
        }

        if let Some(server_id) = server_id {
            let server_environment: Option<Uuid> = sqlx::query_scalar(
                "SELECT environment_id FROM servers WHERE id=$1 AND organization_id=$2 AND project_id=$3",
            )
            .bind(server_id)
            .bind(organization_id)
            .bind(project_id)
            .fetch_optional(&self.pool)
            .await?;
            let server_environment = server_environment.ok_or(ServiceCatalogError::Invalid)?;
            if environment_id.is_some_and(|value| value != server_environment) {
                return Err(ServiceCatalogError::Invalid);
            }
            environment_id = Some(server_environment);
        }

        if let Some(environment_id) = environment_id {
            let valid: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM environments WHERE id=$1 AND organization_id=$2 AND project_id=$3)",
            )
            .bind(environment_id)
            .bind(organization_id)
            .bind(project_id)
            .fetch_one(&self.pool)
            .await?;
            if !valid {
                return Err(ServiceCatalogError::Invalid);
            }
        }

        if let Some(owner_user_id) = owner_user_id {
            let valid: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM users WHERE id=$1 AND organization_id=$2)",
            )
            .bind(owner_user_id)
            .bind(organization_id)
            .fetch_one(&self.pool)
            .await?;
            if !valid {
                return Err(ServiceCatalogError::Invalid);
            }
        }

        Ok(NormalizedService {
            name,
            description,
            service_type,
            runtime,
            repository_id,
            environment_id,
            server_id,
            owner_user_id,
            endpoint_url,
            lifecycle_status,
        })
    }
}

struct NormalizedService {
    name: String,
    description: String,
    service_type: String,
    runtime: Option<String>,
    repository_id: Option<Uuid>,
    environment_id: Option<Uuid>,
    server_id: Option<Uuid>,
    owner_user_id: Option<Uuid>,
    endpoint_url: Option<String>,
    lifecycle_status: String,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/projects/:project_id/services",
            get(list_services).post(create_service),
        )
        .route(
            "/projects/:project_id/services/:service_id",
            get(get_service).put(update_service).delete(delete_service),
        )
}

async fn list_services(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(project_id): Path<Uuid>,
) -> Result<Json<Vec<CatalogService>>, ApiError> {
    let identity = web_identity(&state, &headers).await?;
    Ok(Json(
        state
            .service_catalog
            .list(identity.organization_id, project_id)
            .await
            .map_err(map_catalog)?,
    ))
}

async fn get_service(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((project_id, service_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<CatalogService>, ApiError> {
    let identity = web_identity(&state, &headers).await?;
    Ok(Json(
        state
            .service_catalog
            .get(identity.organization_id, project_id, service_id)
            .await
            .map_err(map_catalog)?,
    ))
}

async fn create_service(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(project_id): Path<Uuid>,
    Json(request): Json<CreateServiceRequest>,
) -> Result<(StatusCode, Json<CatalogService>), ApiError> {
    let identity = web_identity(&state, &headers).await?;
    let service = state
        .service_catalog
        .create(identity, project_id, request)
        .await
        .map_err(map_catalog)?;
    Ok((StatusCode::CREATED, Json(service)))
}

async fn update_service(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((project_id, service_id)): Path<(Uuid, Uuid)>,
    Json(request): Json<UpdateServiceRequest>,
) -> Result<Json<CatalogService>, ApiError> {
    let identity = web_identity(&state, &headers).await?;
    Ok(Json(
        state
            .service_catalog
            .update(identity, project_id, service_id, request)
            .await
            .map_err(map_catalog)?,
    ))
}

async fn delete_service(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((project_id, service_id)): Path<(Uuid, Uuid)>,
) -> Result<StatusCode, ApiError> {
    let identity = web_identity(&state, &headers).await?;
    state
        .service_catalog
        .delete(identity, project_id, service_id)
        .await
        .map_err(map_catalog)?;
    Ok(StatusCode::NO_CONTENT)
}

fn service_from_row(row: sqlx::postgres::PgRow) -> Result<CatalogService, ServiceCatalogError> {
    Ok(CatalogService {
        id: row.get("id"),
        project_id: row.get("project_id"),
        environment_id: row.get("environment_id"),
        server_id: row.get("server_id"),
        repository_id: row.get("repository_id"),
        name: row.get("name"),
        description: row.get("description"),
        service_type: row.get("service_type"),
        runtime: row.get("runtime"),
        owner_user_id: row.get("owner_user_id"),
        endpoint_url: row.get("endpoint_url"),
        lifecycle_status: row.get("lifecycle_status"),
        health_status: row.get("status"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}

fn required_text(value: &str, min: usize, max: usize) -> Result<String, ServiceCatalogError> {
    let value = value.trim();
    if value.len() < min || value.len() > max {
        Err(ServiceCatalogError::Invalid)
    } else {
        Ok(value.to_string())
    }
}

fn optional_text(value: &str, max: usize) -> Result<String, ServiceCatalogError> {
    let value = value.trim();
    if value.len() > max {
        Err(ServiceCatalogError::Invalid)
    } else {
        Ok(value.to_string())
    }
}

fn optional_value(
    value: Option<String>,
    max: usize,
) -> Result<Option<String>, ServiceCatalogError> {
    match value.map(|value| value.trim().to_string()) {
        Some(value) if value.is_empty() => Ok(None),
        Some(value) if value.len() <= max => Ok(Some(value)),
        Some(_) => Err(ServiceCatalogError::Invalid),
        None => Ok(None),
    }
}

fn normalize_endpoint(value: Option<String>) -> Result<Option<String>, ServiceCatalogError> {
    let Some(value) = optional_value(value, 2048)? else {
        return Ok(None);
    };
    let url = Url::parse(&value).map_err(|_| ServiceCatalogError::Invalid)?;
    if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
        return Err(ServiceCatalogError::Invalid);
    }
    Ok(Some(url.to_string()))
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

fn map_catalog(error: ServiceCatalogError) -> ApiError {
    match error {
        ServiceCatalogError::NotFound => api_error(
            StatusCode::NOT_FOUND,
            "SERVICE_NOT_FOUND",
            "service catalog entry not found",
        ),
        ServiceCatalogError::Invalid => api_error(
            StatusCode::BAD_REQUEST,
            "INVALID_REQUEST",
            "invalid service catalog request",
        ),
        other => {
            tracing::error!(error=%other, "service catalog storage error");
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
    fn endpoint_requires_http_or_https() {
        assert!(normalize_endpoint(Some("https://api.example.com".into())).is_ok());
        assert!(normalize_endpoint(Some("ssh://host".into())).is_err());
        assert!(normalize_endpoint(Some("javascript:alert(1)".into())).is_err());
    }

    #[test]
    fn optional_runtime_normalizes_empty_to_none() {
        assert_eq!(optional_value(Some("   ".into()), 120).unwrap(), None);
        assert_eq!(
            optional_value(Some(" Rust / Axum ".into()), 120).unwrap(),
            Some("Rust / Axum".into())
        );
    }
}
