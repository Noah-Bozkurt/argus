use crate::{ApiError, AppState, api_error, web_identity};
use axum::{
    Json, Router,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    routing::{get, post},
};
use chrono::{DateTime, Utc};
use reqwest::Url;
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct DeploymentReleaseStore {
    pool: PgPool,
}

#[derive(Debug, Clone, Serialize)]
pub struct Deployment {
    pub id: Uuid,
    pub project_id: Uuid,
    pub service_id: Uuid,
    pub environment_id: Uuid,
    pub repository_id: Option<Uuid>,
    pub source_commit_sha: Option<String>,
    pub source_version: Option<String>,
    pub provider: String,
    pub status: String,
    pub deployment_url: Option<String>,
    pub error_summary: Option<String>,
    pub notes: String,
    pub previous_deployment_id: Option<Uuid>,
    pub rollback_of_deployment_id: Option<Uuid>,
    pub triggered_by: Uuid,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ReleaseComponent {
    pub id: Uuid,
    pub release_id: Uuid,
    pub service_id: Uuid,
    pub deployment_id: Option<Uuid>,
    pub version: Option<String>,
    pub commit_sha: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Release {
    pub id: Uuid,
    pub project_id: Uuid,
    pub version: String,
    pub name: String,
    pub notes: String,
    pub status: String,
    pub created_by: Uuid,
    pub released_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub components: Vec<ReleaseComponent>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DeploymentReleaseView {
    pub deployments: Vec<Deployment>,
    pub releases: Vec<Release>,
}

#[derive(Debug, Deserialize)]
pub struct CreateDeploymentRequest {
    pub service_id: Uuid,
    pub environment_id: Uuid,
    pub repository_id: Option<Uuid>,
    pub source_commit_sha: Option<String>,
    pub source_version: Option<String>,
    #[serde(default = "default_provider")]
    pub provider: String,
    #[serde(default)]
    pub notes: String,
    pub rollback_of_deployment_id: Option<Uuid>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateDeploymentStatusRequest {
    pub status: String,
    pub deployment_url: Option<String>,
    pub error_summary: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateReleaseRequest {
    pub version: String,
    pub name: String,
    #[serde(default)]
    pub notes: String,
}

#[derive(Debug, Deserialize)]
pub struct AddReleaseComponentRequest {
    pub service_id: Uuid,
    pub deployment_id: Option<Uuid>,
    pub version: Option<String>,
    pub commit_sha: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateReleaseStatusRequest {
    pub status: String,
}

#[derive(Debug, thiserror::Error)]
pub enum DeploymentReleaseError {
    #[error("resource not found")]
    NotFound,
    #[error("invalid deployment or release request")]
    Invalid,
    #[error("deployment or release state conflict")]
    Conflict,
    #[error(transparent)]
    Sql(#[from] sqlx::Error),
}

impl DeploymentReleaseStore {
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
    ) -> Result<(), DeploymentReleaseError> {
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
            Err(DeploymentReleaseError::NotFound)
        }
    }

    pub async fn view(
        &self,
        organization_id: Uuid,
        project_id: Uuid,
    ) -> Result<DeploymentReleaseView, DeploymentReleaseError> {
        self.authorize_project(organization_id, project_id).await?;
        let deployment_rows = sqlx::query(
            "SELECT id,project_id,service_id,environment_id,repository_id,source_commit_sha,source_version,provider,status,deployment_url,error_summary,notes,previous_deployment_id,rollback_of_deployment_id,triggered_by,started_at,finished_at,created_at,updated_at FROM deployments WHERE organization_id=$1 AND project_id=$2 ORDER BY created_at DESC LIMIT 250",
        )
        .bind(organization_id)
        .bind(project_id)
        .fetch_all(&self.pool)
        .await?;
        let deployments = deployment_rows
            .into_iter()
            .map(deployment_from_row)
            .collect::<Result<Vec<_>, _>>()?;

        let release_rows = sqlx::query(
            "SELECT id,project_id,version,name,notes,status,created_by,released_at,created_at,updated_at FROM releases WHERE organization_id=$1 AND project_id=$2 ORDER BY created_at DESC LIMIT 100",
        )
        .bind(organization_id)
        .bind(project_id)
        .fetch_all(&self.pool)
        .await?;
        let component_rows = sqlx::query(
            "SELECT id,release_id,service_id,deployment_id,version,commit_sha,created_at FROM release_components WHERE organization_id=$1 AND project_id=$2 ORDER BY created_at",
        )
        .bind(organization_id)
        .bind(project_id)
        .fetch_all(&self.pool)
        .await?;
        let mut components: HashMap<Uuid, Vec<ReleaseComponent>> = HashMap::new();
        for row in component_rows {
            let component = component_from_row(row)?;
            components
                .entry(component.release_id)
                .or_default()
                .push(component);
        }
        let mut releases = Vec::with_capacity(release_rows.len());
        for row in release_rows {
            let id: Uuid = row.get("id");
            releases.push(Release {
                id,
                project_id: row.get("project_id"),
                version: row.get("version"),
                name: row.get("name"),
                notes: row.get("notes"),
                status: row.get("status"),
                created_by: row.get("created_by"),
                released_at: row.get("released_at"),
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at"),
                components: components.remove(&id).unwrap_or_default(),
            });
        }
        Ok(DeploymentReleaseView {
            deployments,
            releases,
        })
    }

    pub async fn create_deployment(
        &self,
        identity: crate::persistence::WebIdentity,
        project_id: Uuid,
        request: CreateDeploymentRequest,
    ) -> Result<Deployment, DeploymentReleaseError> {
        self.authorize_project(identity.organization_id, project_id)
            .await?;
        let service = sqlx::query(
            "SELECT environment_id,repository_id FROM services WHERE id=$1 AND organization_id=$2 AND project_id=$3",
        )
        .bind(request.service_id)
        .bind(identity.organization_id)
        .bind(project_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(DeploymentReleaseError::NotFound)?;
        let service_environment: Option<Uuid> = service.get("environment_id");
        let service_repository: Option<Uuid> = service.get("repository_id");
        if service_environment.is_some_and(|value| value != request.environment_id) {
            return Err(DeploymentReleaseError::Invalid);
        }
        let environment_exists: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM environments WHERE id=$1 AND organization_id=$2 AND project_id=$3)",
        )
        .bind(request.environment_id)
        .bind(identity.organization_id)
        .bind(project_id)
        .fetch_one(&self.pool)
        .await?;
        if !environment_exists {
            return Err(DeploymentReleaseError::Invalid);
        }
        if let Some(repository_id) = request.repository_id {
            let valid: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM project_repositories WHERE id=$1 AND organization_id=$2 AND project_id=$3)",
            )
            .bind(repository_id)
            .bind(identity.organization_id)
            .bind(project_id)
            .fetch_one(&self.pool)
            .await?;
            if !valid || service_repository.is_some_and(|value| value != repository_id) {
                return Err(DeploymentReleaseError::Invalid);
            }
        }
        let repository_id = request.repository_id.or(service_repository);
        let provider = request.provider.trim().to_lowercase();
        if provider != "manual" {
            return Err(DeploymentReleaseError::Invalid);
        }
        let source_commit_sha = normalize_commit(request.source_commit_sha)?;
        let source_version = optional_value(request.source_version, 120)?;
        let notes = optional_text(&request.notes, 8000)?;

        if let Some(rollback_id) = request.rollback_of_deployment_id {
            let target = self
                .get_deployment(identity.organization_id, project_id, rollback_id)
                .await?;
            if target.service_id != request.service_id
                || target.environment_id != request.environment_id
                || target.status != "SUCCEEDED"
            {
                return Err(DeploymentReleaseError::Invalid);
            }
        }

        let previous_deployment_id: Option<Uuid> = sqlx::query_scalar(
            "SELECT id FROM deployments WHERE organization_id=$1 AND project_id=$2 AND service_id=$3 AND environment_id=$4 AND status='SUCCEEDED' ORDER BY created_at DESC LIMIT 1",
        )
        .bind(identity.organization_id)
        .bind(project_id)
        .bind(request.service_id)
        .bind(request.environment_id)
        .fetch_optional(&self.pool)
        .await?;

        let id = Uuid::new_v4();
        let mut tx = self.pool.begin().await?;
        sqlx::query(
            "INSERT INTO deployments(id,organization_id,project_id,service_id,environment_id,repository_id,source_commit_sha,source_version,provider,status,notes,previous_deployment_id,rollback_of_deployment_id,triggered_by,created_at,updated_at) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,'QUEUED',$10,$11,$12,$13,NOW(),NOW())",
        )
        .bind(id)
        .bind(identity.organization_id)
        .bind(project_id)
        .bind(request.service_id)
        .bind(request.environment_id)
        .bind(repository_id)
        .bind(&source_commit_sha)
        .bind(&source_version)
        .bind(&provider)
        .bind(&notes)
        .bind(previous_deployment_id)
        .bind(request.rollback_of_deployment_id)
        .bind(identity.user_id)
        .execute(&mut *tx)
        .await?;
        touch_project(&mut tx, identity.organization_id, project_id).await?;
        audit_event(
            &mut tx,
            identity,
            project_id,
            "deployment.created",
            serde_json::json!({
                "deployment_id": id,
                "service_id": request.service_id,
                "environment_id": request.environment_id,
                "rollback_of_deployment_id": request.rollback_of_deployment_id
            }),
        )
        .await?;
        tx.commit().await?;
        self.get_deployment(identity.organization_id, project_id, id)
            .await
    }

    pub async fn update_deployment_status(
        &self,
        identity: crate::persistence::WebIdentity,
        project_id: Uuid,
        deployment_id: Uuid,
        request: UpdateDeploymentStatusRequest,
    ) -> Result<Deployment, DeploymentReleaseError> {
        self.authorize_project(identity.organization_id, project_id)
            .await?;
        let deployment = self
            .get_deployment(identity.organization_id, project_id, deployment_id)
            .await?;
        let next = request.status.trim().to_uppercase();
        if !deployment_transition_allowed(&deployment.status, &next) {
            return Err(DeploymentReleaseError::Conflict);
        }
        let deployment_url = normalize_url(request.deployment_url)?;
        let error_summary = optional_value(request.error_summary, 1000)?;
        if next == "FAILED" && error_summary.is_none() {
            return Err(DeploymentReleaseError::Invalid);
        }

        let mut tx = self.pool.begin().await?;
        if next == "SUCCEEDED" {
            if let Some(rollback_id) = deployment.rollback_of_deployment_id {
                let target_status: Option<String> = sqlx::query_scalar(
                    "SELECT status FROM deployments WHERE id=$1 AND organization_id=$2 AND project_id=$3 AND service_id=$4 AND environment_id=$5 FOR UPDATE",
                )
                .bind(rollback_id)
                .bind(identity.organization_id)
                .bind(project_id)
                .bind(deployment.service_id)
                .bind(deployment.environment_id)
                .fetch_optional(&mut *tx)
                .await?;
                if target_status.as_deref() != Some("SUCCEEDED") {
                    tx.rollback().await?;
                    return Err(DeploymentReleaseError::Conflict);
                }
                sqlx::query(
                    "UPDATE deployments SET status='ROLLED_BACK',finished_at=COALESCE(finished_at,NOW()),updated_at=NOW() WHERE id=$1 AND organization_id=$2 AND project_id=$3",
                )
                .bind(rollback_id)
                .bind(identity.organization_id)
                .bind(project_id)
                .execute(&mut *tx)
                .await?;
                audit_event(
                    &mut tx,
                    identity,
                    project_id,
                    "deployment.rolled_back",
                    serde_json::json!({"deployment_id":rollback_id,"rollback_deployment_id":deployment_id}),
                )
                .await?;
            }
        }
        sqlx::query(
            "UPDATE deployments SET status=$1,started_at=CASE WHEN $1='RUNNING' THEN COALESCE(started_at,NOW()) ELSE started_at END,finished_at=CASE WHEN $1 IN ('SUCCEEDED','FAILED','CANCELLED') THEN NOW() ELSE finished_at END,deployment_url=$2,error_summary=$3,updated_at=NOW() WHERE id=$4 AND organization_id=$5 AND project_id=$6",
        )
        .bind(&next)
        .bind(&deployment_url)
        .bind(&error_summary)
        .bind(deployment_id)
        .bind(identity.organization_id)
        .bind(project_id)
        .execute(&mut *tx)
        .await?;
        touch_project(&mut tx, identity.organization_id, project_id).await?;
        audit_event(
            &mut tx,
            identity,
            project_id,
            "deployment.status_changed",
            serde_json::json!({"deployment_id":deployment_id,"from":deployment.status,"to":next}),
        )
        .await?;
        tx.commit().await?;
        self.get_deployment(identity.organization_id, project_id, deployment_id)
            .await
    }

    pub async fn create_release(
        &self,
        identity: crate::persistence::WebIdentity,
        project_id: Uuid,
        request: CreateReleaseRequest,
    ) -> Result<Release, DeploymentReleaseError> {
        self.authorize_project(identity.organization_id, project_id)
            .await?;
        let version = required_text(&request.version, 1, 120)?;
        let name = required_text(&request.name, 1, 200)?;
        let notes = optional_text(&request.notes, 20000)?;
        let id = Uuid::new_v4();
        let mut tx = self.pool.begin().await?;
        let insert = sqlx::query(
            "INSERT INTO releases(id,organization_id,project_id,version,name,notes,status,created_by,created_at,updated_at) VALUES($1,$2,$3,$4,$5,$6,'DRAFT',$7,NOW(),NOW())",
        )
        .bind(id)
        .bind(identity.organization_id)
        .bind(project_id)
        .bind(&version)
        .bind(&name)
        .bind(&notes)
        .bind(identity.user_id)
        .execute(&mut *tx)
        .await;
        if let Err(error) = insert {
            tx.rollback().await?;
            return if is_unique_violation(&error) {
                Err(DeploymentReleaseError::Conflict)
            } else {
                Err(DeploymentReleaseError::Sql(error))
            };
        }
        touch_project(&mut tx, identity.organization_id, project_id).await?;
        audit_event(
            &mut tx,
            identity,
            project_id,
            "release.created",
            serde_json::json!({"release_id":id,"version":version,"name":name}),
        )
        .await?;
        tx.commit().await?;
        self.get_release(identity.organization_id, project_id, id).await
    }

    pub async fn add_release_component(
        &self,
        identity: crate::persistence::WebIdentity,
        project_id: Uuid,
        release_id: Uuid,
        request: AddReleaseComponentRequest,
    ) -> Result<Release, DeploymentReleaseError> {
        self.authorize_project(identity.organization_id, project_id)
            .await?;
        let release = self
            .get_release(identity.organization_id, project_id, release_id)
            .await?;
        if release.status != "DRAFT" {
            return Err(DeploymentReleaseError::Conflict);
        }
        let service_exists: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM services WHERE id=$1 AND organization_id=$2 AND project_id=$3)",
        )
        .bind(request.service_id)
        .bind(identity.organization_id)
        .bind(project_id)
        .fetch_one(&self.pool)
        .await?;
        if !service_exists {
            return Err(DeploymentReleaseError::Invalid);
        }

        let mut version = optional_value(request.version, 120)?;
        let mut commit_sha = normalize_commit(request.commit_sha)?;
        if let Some(deployment_id) = request.deployment_id {
            let deployment = self
                .get_deployment(identity.organization_id, project_id, deployment_id)
                .await?;
            if deployment.service_id != request.service_id || deployment.status != "SUCCEEDED" {
                return Err(DeploymentReleaseError::Invalid);
            }
            if version.is_none() {
                version = deployment.source_version;
            }
            if commit_sha.is_none() {
                commit_sha = deployment.source_commit_sha;
            }
        }

        let id = Uuid::new_v4();
        let mut tx = self.pool.begin().await?;
        let insert = sqlx::query(
            "INSERT INTO release_components(id,organization_id,project_id,release_id,service_id,deployment_id,version,commit_sha,created_at) VALUES($1,$2,$3,$4,$5,$6,$7,$8,NOW())",
        )
        .bind(id)
        .bind(identity.organization_id)
        .bind(project_id)
        .bind(release_id)
        .bind(request.service_id)
        .bind(request.deployment_id)
        .bind(&version)
        .bind(&commit_sha)
        .execute(&mut *tx)
        .await;
        if let Err(error) = insert {
            tx.rollback().await?;
            return if is_unique_violation(&error) {
                Err(DeploymentReleaseError::Conflict)
            } else {
                Err(DeploymentReleaseError::Sql(error))
            };
        }
        touch_project(&mut tx, identity.organization_id, project_id).await?;
        audit_event(
            &mut tx,
            identity,
            project_id,
            "release.component_added",
            serde_json::json!({
                "release_id":release_id,
                "component_id":id,
                "service_id":request.service_id,
                "deployment_id":request.deployment_id
            }),
        )
        .await?;
        tx.commit().await?;
        self.get_release(identity.organization_id, project_id, release_id)
            .await
    }

    pub async fn update_release_status(
        &self,
        identity: crate::persistence::WebIdentity,
        project_id: Uuid,
        release_id: Uuid,
        request: UpdateReleaseStatusRequest,
    ) -> Result<Release, DeploymentReleaseError> {
        self.authorize_project(identity.organization_id, project_id)
            .await?;
        let release = self
            .get_release(identity.organization_id, project_id, release_id)
            .await?;
        let next = request.status.trim().to_uppercase();
        if !release_transition_allowed(&release.status, &next) {
            return Err(DeploymentReleaseError::Conflict);
        }
        if matches!(next.as_str(), "READY" | "RELEASED") {
            self.validate_release_ready(identity.organization_id, project_id, release_id)
                .await?;
        }
        let mut tx = self.pool.begin().await?;
        sqlx::query(
            "UPDATE releases SET status=$1,released_at=CASE WHEN $1='RELEASED' THEN NOW() ELSE released_at END,updated_at=NOW() WHERE id=$2 AND organization_id=$3 AND project_id=$4",
        )
        .bind(&next)
        .bind(release_id)
        .bind(identity.organization_id)
        .bind(project_id)
        .execute(&mut *tx)
        .await?;
        touch_project(&mut tx, identity.organization_id, project_id).await?;
        audit_event(
            &mut tx,
            identity,
            project_id,
            "release.status_changed",
            serde_json::json!({"release_id":release_id,"from":release.status,"to":next}),
        )
        .await?;
        tx.commit().await?;
        self.get_release(identity.organization_id, project_id, release_id)
            .await
    }

    async fn validate_release_ready(
        &self,
        organization_id: Uuid,
        project_id: Uuid,
        release_id: Uuid,
    ) -> Result<(), DeploymentReleaseError> {
        let row = sqlx::query(
            "SELECT COUNT(*) AS total,COUNT(*) FILTER (WHERE d.id IS NOT NULL AND d.status='SUCCEEDED') AS successful FROM release_components rc LEFT JOIN deployments d ON d.id=rc.deployment_id AND d.organization_id=rc.organization_id AND d.project_id=rc.project_id WHERE rc.organization_id=$1 AND rc.project_id=$2 AND rc.release_id=$3",
        )
        .bind(organization_id)
        .bind(project_id)
        .bind(release_id)
        .fetch_one(&self.pool)
        .await?;
        let total: i64 = row.get("total");
        let successful: i64 = row.get("successful");
        if total == 0 || total != successful {
            Err(DeploymentReleaseError::Conflict)
        } else {
            Ok(())
        }
    }

    async fn get_deployment(
        &self,
        organization_id: Uuid,
        project_id: Uuid,
        deployment_id: Uuid,
    ) -> Result<Deployment, DeploymentReleaseError> {
        let row = sqlx::query(
            "SELECT id,project_id,service_id,environment_id,repository_id,source_commit_sha,source_version,provider,status,deployment_url,error_summary,notes,previous_deployment_id,rollback_of_deployment_id,triggered_by,started_at,finished_at,created_at,updated_at FROM deployments WHERE id=$1 AND organization_id=$2 AND project_id=$3",
        )
        .bind(deployment_id)
        .bind(organization_id)
        .bind(project_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(DeploymentReleaseError::NotFound)?;
        deployment_from_row(row)
    }

    async fn get_release(
        &self,
        organization_id: Uuid,
        project_id: Uuid,
        release_id: Uuid,
    ) -> Result<Release, DeploymentReleaseError> {
        let row = sqlx::query(
            "SELECT id,project_id,version,name,notes,status,created_by,released_at,created_at,updated_at FROM releases WHERE id=$1 AND organization_id=$2 AND project_id=$3",
        )
        .bind(release_id)
        .bind(organization_id)
        .bind(project_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(DeploymentReleaseError::NotFound)?;
        let component_rows = sqlx::query(
            "SELECT id,release_id,service_id,deployment_id,version,commit_sha,created_at FROM release_components WHERE release_id=$1 AND organization_id=$2 AND project_id=$3 ORDER BY created_at",
        )
        .bind(release_id)
        .bind(organization_id)
        .bind(project_id)
        .fetch_all(&self.pool)
        .await?;
        let components = component_rows
            .into_iter()
            .map(component_from_row)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Release {
            id: row.get("id"),
            project_id: row.get("project_id"),
            version: row.get("version"),
            name: row.get("name"),
            notes: row.get("notes"),
            status: row.get("status"),
            created_by: row.get("created_by"),
            released_at: row.get("released_at"),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
            components,
        })
    }
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/projects/:project_id/deployments-releases",
            get(get_deployment_release_view),
        )
        .route(
            "/projects/:project_id/deployments",
            post(create_deployment),
        )
        .route(
            "/projects/:project_id/deployments/:deployment_id/status",
            axum::routing::put(update_deployment_status),
        )
        .route("/projects/:project_id/releases", post(create_release))
        .route(
            "/projects/:project_id/releases/:release_id/components",
            post(add_release_component),
        )
        .route(
            "/projects/:project_id/releases/:release_id/status",
            axum::routing::put(update_release_status),
        )
}

async fn get_deployment_release_view(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(project_id): Path<Uuid>,
) -> Result<Json<DeploymentReleaseView>, ApiError> {
    let identity = web_identity(&state, &headers).await?;
    Ok(Json(
        state
            .deployments_releases
            .view(identity.organization_id, project_id)
            .await
            .map_err(map_deployment_release)?,
    ))
}

async fn create_deployment(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(project_id): Path<Uuid>,
    Json(request): Json<CreateDeploymentRequest>,
) -> Result<(StatusCode, Json<Deployment>), ApiError> {
    let identity = web_identity(&state, &headers).await?;
    let deployment = state
        .deployments_releases
        .create_deployment(identity, project_id, request)
        .await
        .map_err(map_deployment_release)?;
    Ok((StatusCode::CREATED, Json(deployment)))
}

async fn update_deployment_status(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((project_id, deployment_id)): Path<(Uuid, Uuid)>,
    Json(request): Json<UpdateDeploymentStatusRequest>,
) -> Result<Json<Deployment>, ApiError> {
    let identity = web_identity(&state, &headers).await?;
    Ok(Json(
        state
            .deployments_releases
            .update_deployment_status(identity, project_id, deployment_id, request)
            .await
            .map_err(map_deployment_release)?,
    ))
}

async fn create_release(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(project_id): Path<Uuid>,
    Json(request): Json<CreateReleaseRequest>,
) -> Result<(StatusCode, Json<Release>), ApiError> {
    let identity = web_identity(&state, &headers).await?;
    let release = state
        .deployments_releases
        .create_release(identity, project_id, request)
        .await
        .map_err(map_deployment_release)?;
    Ok((StatusCode::CREATED, Json(release)))
}

async fn add_release_component(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((project_id, release_id)): Path<(Uuid, Uuid)>,
    Json(request): Json<AddReleaseComponentRequest>,
) -> Result<Json<Release>, ApiError> {
    let identity = web_identity(&state, &headers).await?;
    Ok(Json(
        state
            .deployments_releases
            .add_release_component(identity, project_id, release_id, request)
            .await
            .map_err(map_deployment_release)?,
    ))
}

async fn update_release_status(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((project_id, release_id)): Path<(Uuid, Uuid)>,
    Json(request): Json<UpdateReleaseStatusRequest>,
) -> Result<Json<Release>, ApiError> {
    let identity = web_identity(&state, &headers).await?;
    Ok(Json(
        state
            .deployments_releases
            .update_release_status(identity, project_id, release_id, request)
            .await
            .map_err(map_deployment_release)?,
    ))
}

fn deployment_from_row(row: sqlx::postgres::PgRow) -> Result<Deployment, DeploymentReleaseError> {
    Ok(Deployment {
        id: row.get("id"),
        project_id: row.get("project_id"),
        service_id: row.get("service_id"),
        environment_id: row.get("environment_id"),
        repository_id: row.get("repository_id"),
        source_commit_sha: row.get("source_commit_sha"),
        source_version: row.get("source_version"),
        provider: row.get("provider"),
        status: row.get("status"),
        deployment_url: row.get("deployment_url"),
        error_summary: row.get("error_summary"),
        notes: row.get("notes"),
        previous_deployment_id: row.get("previous_deployment_id"),
        rollback_of_deployment_id: row.get("rollback_of_deployment_id"),
        triggered_by: row.get("triggered_by"),
        started_at: row.get("started_at"),
        finished_at: row.get("finished_at"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}

fn component_from_row(
    row: sqlx::postgres::PgRow,
) -> Result<ReleaseComponent, DeploymentReleaseError> {
    Ok(ReleaseComponent {
        id: row.get("id"),
        release_id: row.get("release_id"),
        service_id: row.get("service_id"),
        deployment_id: row.get("deployment_id"),
        version: row.get("version"),
        commit_sha: row.get("commit_sha"),
        created_at: row.get("created_at"),
    })
}

fn default_provider() -> String {
    "manual".into()
}

fn deployment_transition_allowed(current: &str, next: &str) -> bool {
    matches!(
        (current, next),
        ("QUEUED", "RUNNING")
            | ("QUEUED", "CANCELLED")
            | ("RUNNING", "SUCCEEDED")
            | ("RUNNING", "FAILED")
            | ("RUNNING", "CANCELLED")
    )
}

fn release_transition_allowed(current: &str, next: &str) -> bool {
    matches!(
        (current, next),
        ("DRAFT", "READY")
            | ("DRAFT", "FAILED")
            | ("READY", "RELEASED")
            | ("READY", "FAILED")
            | ("RELEASED", "ROLLED_BACK")
    )
}

fn required_text(
    value: &str,
    min: usize,
    max: usize,
) -> Result<String, DeploymentReleaseError> {
    let value = value.trim();
    if value.len() < min || value.len() > max {
        Err(DeploymentReleaseError::Invalid)
    } else {
        Ok(value.to_string())
    }
}

fn optional_text(value: &str, max: usize) -> Result<String, DeploymentReleaseError> {
    let value = value.trim();
    if value.len() > max {
        Err(DeploymentReleaseError::Invalid)
    } else {
        Ok(value.to_string())
    }
}

fn optional_value(
    value: Option<String>,
    max: usize,
) -> Result<Option<String>, DeploymentReleaseError> {
    match value.map(|value| value.trim().to_string()) {
        Some(value) if value.is_empty() => Ok(None),
        Some(value) if value.len() <= max => Ok(Some(value)),
        Some(_) => Err(DeploymentReleaseError::Invalid),
        None => Ok(None),
    }
}

fn normalize_commit(value: Option<String>) -> Result<Option<String>, DeploymentReleaseError> {
    let Some(value) = optional_value(value, 64)? else {
        return Ok(None);
    };
    if value.len() < 7 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(DeploymentReleaseError::Invalid);
    }
    Ok(Some(value.to_ascii_lowercase()))
}

fn normalize_url(value: Option<String>) -> Result<Option<String>, DeploymentReleaseError> {
    let Some(value) = optional_value(value, 2048)? else {
        return Ok(None);
    };
    let url = Url::parse(&value).map_err(|_| DeploymentReleaseError::Invalid)?;
    if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
        return Err(DeploymentReleaseError::Invalid);
    }
    Ok(Some(url.to_string()))
}

fn is_unique_violation(error: &sqlx::Error) -> bool {
    matches!(error, sqlx::Error::Database(database) if database.code().as_deref() == Some("23505"))
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

fn map_deployment_release(error: DeploymentReleaseError) -> ApiError {
    match error {
        DeploymentReleaseError::NotFound => api_error(
            StatusCode::NOT_FOUND,
            "DEPLOYMENT_RESOURCE_NOT_FOUND",
            "deployment or release resource not found",
        ),
        DeploymentReleaseError::Invalid => api_error(
            StatusCode::BAD_REQUEST,
            "INVALID_REQUEST",
            "invalid deployment or release request",
        ),
        DeploymentReleaseError::Conflict => api_error(
            StatusCode::CONFLICT,
            "OPERATION_CONFLICT",
            "deployment or release state conflicts with this operation",
        ),
        other => {
            tracing::error!(error=%other, "deployment/release storage error");
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
    fn deployment_transitions_are_narrow() {
        assert!(deployment_transition_allowed("QUEUED", "RUNNING"));
        assert!(deployment_transition_allowed("RUNNING", "SUCCEEDED"));
        assert!(!deployment_transition_allowed("SUCCEEDED", "RUNNING"));
        assert!(!deployment_transition_allowed("QUEUED", "SUCCEEDED"));
    }

    #[test]
    fn release_transitions_are_narrow() {
        assert!(release_transition_allowed("DRAFT", "READY"));
        assert!(release_transition_allowed("READY", "RELEASED"));
        assert!(release_transition_allowed("RELEASED", "ROLLED_BACK"));
        assert!(!release_transition_allowed("DRAFT", "RELEASED"));
    }

    #[test]
    fn commits_are_bounded_hex() {
        assert_eq!(
            normalize_commit(Some("ABCDEF123".into())).unwrap(),
            Some("abcdef123".into())
        );
        assert!(normalize_commit(Some("xyz1234".into())).is_err());
        assert!(normalize_commit(Some("abc".into())).is_err());
    }

    #[test]
    fn deployment_urls_require_http_or_https() {
        assert!(normalize_url(Some("https://preview.example.com".into())).is_ok());
        assert!(normalize_url(Some("ssh://host".into())).is_err());
    }
}
