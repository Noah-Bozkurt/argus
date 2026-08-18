use crate::{ApiError, AppState, api_error, web_identity};
use axum::{
    Json, Router,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    routing::get,
};
use chrono::{DateTime, NaiveDate, TimeZone, Utc};
use reqwest::Url;
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct SiteDomainStore {
    pool: PgPool,
}

#[derive(Debug, Clone, Serialize)]
pub struct Site {
    pub id: Uuid,
    pub project_id: Uuid,
    pub service_id: Option<Uuid>,
    pub repository_id: Option<Uuid>,
    pub environment_id: Option<Uuid>,
    pub name: String,
    pub description: String,
    pub framework: Option<String>,
    pub canonical_url: Option<String>,
    pub lifecycle_status: String,
    pub health_status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Domain {
    pub id: Uuid,
    pub project_id: Uuid,
    pub site_id: Option<Uuid>,
    pub hostname: String,
    pub registrar: Option<String>,
    pub dns_provider: Option<String>,
    pub routing_mode: String,
    pub is_primary: bool,
    pub expires_at: Option<DateTime<Utc>>,
    pub tls_status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SiteDomainView {
    pub sites: Vec<Site>,
    pub domains: Vec<Domain>,
}

#[derive(Debug, Deserialize)]
pub struct CreateSiteRequest {
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub service_id: Option<Uuid>,
    pub repository_id: Option<Uuid>,
    pub environment_id: Option<Uuid>,
    pub framework: Option<String>,
    pub canonical_url: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateSiteRequest {
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub service_id: Option<Uuid>,
    pub repository_id: Option<Uuid>,
    pub environment_id: Option<Uuid>,
    pub framework: Option<String>,
    pub canonical_url: Option<String>,
    pub lifecycle_status: String,
}

#[derive(Debug, Deserialize)]
pub struct DomainRequest {
    pub site_id: Option<Uuid>,
    pub hostname: String,
    pub registrar: Option<String>,
    pub dns_provider: Option<String>,
    pub routing_mode: String,
    #[serde(default)]
    pub is_primary: bool,
    pub expires_at: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum SiteDomainError {
    #[error("site or domain not found")]
    NotFound,
    #[error("invalid site or domain request")]
    Invalid,
    #[error("site or domain conflict")]
    Conflict,
    #[error(transparent)]
    Sql(#[from] sqlx::Error),
}

impl SiteDomainStore {
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
    ) -> Result<(), SiteDomainError> {
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
            Err(SiteDomainError::NotFound)
        }
    }

    pub async fn view(
        &self,
        organization_id: Uuid,
        project_id: Uuid,
    ) -> Result<SiteDomainView, SiteDomainError> {
        self.authorize_project(organization_id, project_id).await?;
        let site_rows = sqlx::query(
            "SELECT id,project_id,service_id,repository_id,environment_id,name,description,framework,canonical_url,lifecycle_status,health_status,created_at,updated_at FROM sites WHERE organization_id=$1 AND project_id=$2 ORDER BY CASE lifecycle_status WHEN 'ACTIVE' THEN 0 WHEN 'PAUSED' THEN 1 ELSE 2 END,updated_at DESC,name",
        )
        .bind(organization_id)
        .bind(project_id)
        .fetch_all(&self.pool)
        .await?;
        let domain_rows = sqlx::query(
            "SELECT id,project_id,site_id,hostname,registrar,dns_provider,routing_mode,is_primary,expires_at,tls_status,created_at,updated_at FROM domains WHERE organization_id=$1 AND project_id=$2 ORDER BY is_primary DESC,hostname",
        )
        .bind(organization_id)
        .bind(project_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(SiteDomainView {
            sites: site_rows
                .into_iter()
                .map(site_from_row)
                .collect::<Result<Vec<_>, _>>()?,
            domains: domain_rows
                .into_iter()
                .map(domain_from_row)
                .collect::<Result<Vec<_>, _>>()?,
        })
    }

    pub async fn create_site(
        &self,
        identity: crate::persistence::WebIdentity,
        project_id: Uuid,
        request: CreateSiteRequest,
    ) -> Result<Site, SiteDomainError> {
        self.authorize_project(identity.organization_id, project_id)
            .await?;
        let normalized = self
            .normalize_site(
                identity.organization_id,
                project_id,
                request.name,
                request.description,
                request.service_id,
                request.repository_id,
                request.environment_id,
                request.framework,
                request.canonical_url,
                "ACTIVE".into(),
            )
            .await?;
        let id = Uuid::new_v4();
        let mut tx = self.pool.begin().await?;
        sqlx::query(
            "INSERT INTO sites(id,organization_id,project_id,service_id,repository_id,environment_id,name,description,framework,canonical_url,lifecycle_status,health_status,created_at,updated_at) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,'UNKNOWN',NOW(),NOW())",
        )
        .bind(id)
        .bind(identity.organization_id)
        .bind(project_id)
        .bind(normalized.service_id)
        .bind(normalized.repository_id)
        .bind(normalized.environment_id)
        .bind(&normalized.name)
        .bind(&normalized.description)
        .bind(&normalized.framework)
        .bind(&normalized.canonical_url)
        .bind(&normalized.lifecycle_status)
        .execute(&mut *tx)
        .await?;
        touch_project(&mut tx, identity.organization_id, project_id).await?;
        audit_event(
            &mut tx,
            identity,
            project_id,
            "site.created",
            serde_json::json!({"site_id":id,"name":normalized.name}),
        )
        .await?;
        tx.commit().await?;
        self.get_site(identity.organization_id, project_id, id)
            .await
    }

    pub async fn update_site(
        &self,
        identity: crate::persistence::WebIdentity,
        project_id: Uuid,
        site_id: Uuid,
        request: UpdateSiteRequest,
    ) -> Result<Site, SiteDomainError> {
        self.authorize_project(identity.organization_id, project_id)
            .await?;
        self.get_site(identity.organization_id, project_id, site_id)
            .await?;
        let normalized = self
            .normalize_site(
                identity.organization_id,
                project_id,
                request.name,
                request.description,
                request.service_id,
                request.repository_id,
                request.environment_id,
                request.framework,
                request.canonical_url,
                request.lifecycle_status,
            )
            .await?;
        let mut tx = self.pool.begin().await?;
        sqlx::query(
            "UPDATE sites SET service_id=$1,repository_id=$2,environment_id=$3,name=$4,description=$5,framework=$6,canonical_url=$7,lifecycle_status=$8,updated_at=NOW() WHERE id=$9 AND organization_id=$10 AND project_id=$11",
        )
        .bind(normalized.service_id)
        .bind(normalized.repository_id)
        .bind(normalized.environment_id)
        .bind(&normalized.name)
        .bind(&normalized.description)
        .bind(&normalized.framework)
        .bind(&normalized.canonical_url)
        .bind(&normalized.lifecycle_status)
        .bind(site_id)
        .bind(identity.organization_id)
        .bind(project_id)
        .execute(&mut *tx)
        .await?;
        touch_project(&mut tx, identity.organization_id, project_id).await?;
        audit_event(
            &mut tx,
            identity,
            project_id,
            "site.updated",
            serde_json::json!({"site_id":site_id,"name":normalized.name,"lifecycle_status":normalized.lifecycle_status}),
        )
        .await?;
        tx.commit().await?;
        self.get_site(identity.organization_id, project_id, site_id)
            .await
    }

    pub async fn delete_site(
        &self,
        identity: crate::persistence::WebIdentity,
        project_id: Uuid,
        site_id: Uuid,
    ) -> Result<(), SiteDomainError> {
        self.authorize_project(identity.organization_id, project_id)
            .await?;
        let site = self
            .get_site(identity.organization_id, project_id, site_id)
            .await?;
        let has_domains: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM domains WHERE site_id=$1 AND organization_id=$2 AND project_id=$3)",
        )
        .bind(site_id)
        .bind(identity.organization_id)
        .bind(project_id)
        .fetch_one(&self.pool)
        .await?;
        if has_domains {
            return Err(SiteDomainError::Conflict);
        }
        let mut tx = self.pool.begin().await?;
        sqlx::query("DELETE FROM sites WHERE id=$1 AND organization_id=$2 AND project_id=$3")
            .bind(site_id)
            .bind(identity.organization_id)
            .bind(project_id)
            .execute(&mut *tx)
            .await?;
        touch_project(&mut tx, identity.organization_id, project_id).await?;
        audit_event(
            &mut tx,
            identity,
            project_id,
            "site.deleted",
            serde_json::json!({"site_id":site_id,"name":site.name}),
        )
        .await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn create_domain(
        &self,
        identity: crate::persistence::WebIdentity,
        project_id: Uuid,
        request: DomainRequest,
    ) -> Result<Domain, SiteDomainError> {
        self.authorize_project(identity.organization_id, project_id)
            .await?;
        let normalized = self
            .normalize_domain(identity.organization_id, project_id, request)
            .await?;
        let id = Uuid::new_v4();
        let mut tx = self.pool.begin().await?;
        let insert = sqlx::query(
            "INSERT INTO domains(id,organization_id,project_id,site_id,hostname,registrar,dns_provider,routing_mode,is_primary,expires_at,tls_status,created_at,updated_at) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,'UNKNOWN',NOW(),NOW())",
        )
        .bind(id)
        .bind(identity.organization_id)
        .bind(project_id)
        .bind(normalized.site_id)
        .bind(&normalized.hostname)
        .bind(&normalized.registrar)
        .bind(&normalized.dns_provider)
        .bind(&normalized.routing_mode)
        .bind(normalized.is_primary)
        .bind(normalized.expires_at)
        .execute(&mut *tx)
        .await;
        if let Err(error) = insert {
            tx.rollback().await?;
            return map_write_error(error);
        }
        touch_project(&mut tx, identity.organization_id, project_id).await?;
        audit_event(
            &mut tx,
            identity,
            project_id,
            "domain.created",
            serde_json::json!({"domain_id":id,"hostname":normalized.hostname,"site_id":normalized.site_id}),
        )
        .await?;
        tx.commit().await?;
        self.get_domain(identity.organization_id, project_id, id)
            .await
    }

    pub async fn update_domain(
        &self,
        identity: crate::persistence::WebIdentity,
        project_id: Uuid,
        domain_id: Uuid,
        request: DomainRequest,
    ) -> Result<Domain, SiteDomainError> {
        self.authorize_project(identity.organization_id, project_id)
            .await?;
        self.get_domain(identity.organization_id, project_id, domain_id)
            .await?;
        let normalized = self
            .normalize_domain(identity.organization_id, project_id, request)
            .await?;
        let mut tx = self.pool.begin().await?;
        let update = sqlx::query(
            "UPDATE domains SET site_id=$1,hostname=$2,registrar=$3,dns_provider=$4,routing_mode=$5,is_primary=$6,expires_at=$7,updated_at=NOW() WHERE id=$8 AND organization_id=$9 AND project_id=$10",
        )
        .bind(normalized.site_id)
        .bind(&normalized.hostname)
        .bind(&normalized.registrar)
        .bind(&normalized.dns_provider)
        .bind(&normalized.routing_mode)
        .bind(normalized.is_primary)
        .bind(normalized.expires_at)
        .bind(domain_id)
        .bind(identity.organization_id)
        .bind(project_id)
        .execute(&mut *tx)
        .await;
        if let Err(error) = update {
            tx.rollback().await?;
            return map_write_error(error);
        }
        touch_project(&mut tx, identity.organization_id, project_id).await?;
        audit_event(
            &mut tx,
            identity,
            project_id,
            "domain.updated",
            serde_json::json!({"domain_id":domain_id,"hostname":normalized.hostname,"site_id":normalized.site_id,"routing_mode":normalized.routing_mode}),
        )
        .await?;
        tx.commit().await?;
        self.get_domain(identity.organization_id, project_id, domain_id)
            .await
    }

    pub async fn delete_domain(
        &self,
        identity: crate::persistence::WebIdentity,
        project_id: Uuid,
        domain_id: Uuid,
    ) -> Result<(), SiteDomainError> {
        self.authorize_project(identity.organization_id, project_id)
            .await?;
        let domain = self
            .get_domain(identity.organization_id, project_id, domain_id)
            .await?;
        let mut tx = self.pool.begin().await?;
        sqlx::query("DELETE FROM domains WHERE id=$1 AND organization_id=$2 AND project_id=$3")
            .bind(domain_id)
            .bind(identity.organization_id)
            .bind(project_id)
            .execute(&mut *tx)
            .await?;
        touch_project(&mut tx, identity.organization_id, project_id).await?;
        audit_event(
            &mut tx,
            identity,
            project_id,
            "domain.deleted",
            serde_json::json!({"domain_id":domain_id,"hostname":domain.hostname}),
        )
        .await?;
        tx.commit().await?;
        Ok(())
    }

    async fn get_site(
        &self,
        organization_id: Uuid,
        project_id: Uuid,
        site_id: Uuid,
    ) -> Result<Site, SiteDomainError> {
        let row = sqlx::query(
            "SELECT id,project_id,service_id,repository_id,environment_id,name,description,framework,canonical_url,lifecycle_status,health_status,created_at,updated_at FROM sites WHERE id=$1 AND organization_id=$2 AND project_id=$3",
        )
        .bind(site_id)
        .bind(organization_id)
        .bind(project_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(SiteDomainError::NotFound)?;
        site_from_row(row)
    }

    async fn get_domain(
        &self,
        organization_id: Uuid,
        project_id: Uuid,
        domain_id: Uuid,
    ) -> Result<Domain, SiteDomainError> {
        let row = sqlx::query(
            "SELECT id,project_id,site_id,hostname,registrar,dns_provider,routing_mode,is_primary,expires_at,tls_status,created_at,updated_at FROM domains WHERE id=$1 AND organization_id=$2 AND project_id=$3",
        )
        .bind(domain_id)
        .bind(organization_id)
        .bind(project_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(SiteDomainError::NotFound)?;
        domain_from_row(row)
    }

    #[allow(clippy::too_many_arguments)]
    async fn normalize_site(
        &self,
        organization_id: Uuid,
        project_id: Uuid,
        name: String,
        description: String,
        service_id: Option<Uuid>,
        mut repository_id: Option<Uuid>,
        mut environment_id: Option<Uuid>,
        framework: Option<String>,
        canonical_url: Option<String>,
        lifecycle_status: String,
    ) -> Result<NormalizedSite, SiteDomainError> {
        let name = required_text(&name, 1, 160)?;
        let description = optional_text(&description, 8000)?;
        let framework = optional_value(framework, 120)?;
        let canonical_url = normalize_http_url(canonical_url)?;
        let lifecycle_status = lifecycle_status.trim().to_uppercase();
        if !matches!(lifecycle_status.as_str(), "ACTIVE" | "PAUSED" | "ARCHIVED") {
            return Err(SiteDomainError::Invalid);
        }

        if let Some(service_id) = service_id {
            let row = sqlx::query(
                "SELECT repository_id,environment_id FROM services WHERE id=$1 AND organization_id=$2 AND project_id=$3",
            )
            .bind(service_id)
            .bind(organization_id)
            .bind(project_id)
            .fetch_optional(&self.pool)
            .await?
            .ok_or(SiteDomainError::Invalid)?;
            let service_repository: Option<Uuid> = row.get("repository_id");
            let service_environment: Option<Uuid> = row.get("environment_id");
            if repository_id.is_some()
                && service_repository.is_some()
                && repository_id != service_repository
            {
                return Err(SiteDomainError::Invalid);
            }
            if environment_id.is_some()
                && service_environment.is_some()
                && environment_id != service_environment
            {
                return Err(SiteDomainError::Invalid);
            }
            repository_id = repository_id.or(service_repository);
            environment_id = environment_id.or(service_environment);
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
                return Err(SiteDomainError::Invalid);
            }
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
                return Err(SiteDomainError::Invalid);
            }
        }

        Ok(NormalizedSite {
            service_id,
            repository_id,
            environment_id,
            name,
            description,
            framework,
            canonical_url,
            lifecycle_status,
        })
    }

    async fn normalize_domain(
        &self,
        organization_id: Uuid,
        project_id: Uuid,
        request: DomainRequest,
    ) -> Result<NormalizedDomain, SiteDomainError> {
        let hostname = normalize_hostname(&request.hostname)?;
        let registrar = optional_value(request.registrar, 120)?;
        let dns_provider = optional_value(request.dns_provider, 120)?;
        let routing_mode = request.routing_mode.trim().to_uppercase();
        if !matches!(
            routing_mode.as_str(),
            "DIRECT" | "CLOUDFLARE_PROXY" | "CLOUDFLARE_TUNNEL"
        ) {
            return Err(SiteDomainError::Invalid);
        }
        if request.is_primary && request.site_id.is_none() {
            return Err(SiteDomainError::Invalid);
        }
        if let Some(site_id) = request.site_id {
            self.get_site(organization_id, project_id, site_id).await?;
        }
        let expires_at = normalize_expiry(request.expires_at)?;
        Ok(NormalizedDomain {
            site_id: request.site_id,
            hostname,
            registrar,
            dns_provider,
            routing_mode,
            is_primary: request.is_primary,
            expires_at,
        })
    }
}

struct NormalizedSite {
    service_id: Option<Uuid>,
    repository_id: Option<Uuid>,
    environment_id: Option<Uuid>,
    name: String,
    description: String,
    framework: Option<String>,
    canonical_url: Option<String>,
    lifecycle_status: String,
}

struct NormalizedDomain {
    site_id: Option<Uuid>,
    hostname: String,
    registrar: Option<String>,
    dns_provider: Option<String>,
    routing_mode: String,
    is_primary: bool,
    expires_at: Option<DateTime<Utc>>,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/projects/:project_id/sites-domains",
            get(get_sites_domains),
        )
        .route(
            "/projects/:project_id/sites",
            axum::routing::post(create_site),
        )
        .route(
            "/projects/:project_id/sites/:site_id",
            axum::routing::put(update_site).delete(delete_site),
        )
        .route(
            "/projects/:project_id/domains",
            axum::routing::post(create_domain),
        )
        .route(
            "/projects/:project_id/domains/:domain_id",
            axum::routing::put(update_domain).delete(delete_domain),
        )
}

async fn get_sites_domains(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(project_id): Path<Uuid>,
) -> Result<Json<SiteDomainView>, ApiError> {
    let identity = web_identity(&state, &headers).await?;
    Ok(Json(
        state
            .sites_domains
            .view(identity.organization_id, project_id)
            .await
            .map_err(map_sites_domains)?,
    ))
}

async fn create_site(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(project_id): Path<Uuid>,
    Json(request): Json<CreateSiteRequest>,
) -> Result<(StatusCode, Json<Site>), ApiError> {
    let identity = web_identity(&state, &headers).await?;
    let site = state
        .sites_domains
        .create_site(identity, project_id, request)
        .await
        .map_err(map_sites_domains)?;
    Ok((StatusCode::CREATED, Json(site)))
}

async fn update_site(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((project_id, site_id)): Path<(Uuid, Uuid)>,
    Json(request): Json<UpdateSiteRequest>,
) -> Result<Json<Site>, ApiError> {
    let identity = web_identity(&state, &headers).await?;
    Ok(Json(
        state
            .sites_domains
            .update_site(identity, project_id, site_id, request)
            .await
            .map_err(map_sites_domains)?,
    ))
}

async fn delete_site(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((project_id, site_id)): Path<(Uuid, Uuid)>,
) -> Result<StatusCode, ApiError> {
    let identity = web_identity(&state, &headers).await?;
    state
        .sites_domains
        .delete_site(identity, project_id, site_id)
        .await
        .map_err(map_sites_domains)?;
    Ok(StatusCode::NO_CONTENT)
}

async fn create_domain(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(project_id): Path<Uuid>,
    Json(request): Json<DomainRequest>,
) -> Result<(StatusCode, Json<Domain>), ApiError> {
    let identity = web_identity(&state, &headers).await?;
    let domain = state
        .sites_domains
        .create_domain(identity, project_id, request)
        .await
        .map_err(map_sites_domains)?;
    Ok((StatusCode::CREATED, Json(domain)))
}

async fn update_domain(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((project_id, domain_id)): Path<(Uuid, Uuid)>,
    Json(request): Json<DomainRequest>,
) -> Result<Json<Domain>, ApiError> {
    let identity = web_identity(&state, &headers).await?;
    Ok(Json(
        state
            .sites_domains
            .update_domain(identity, project_id, domain_id, request)
            .await
            .map_err(map_sites_domains)?,
    ))
}

async fn delete_domain(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((project_id, domain_id)): Path<(Uuid, Uuid)>,
) -> Result<StatusCode, ApiError> {
    let identity = web_identity(&state, &headers).await?;
    state
        .sites_domains
        .delete_domain(identity, project_id, domain_id)
        .await
        .map_err(map_sites_domains)?;
    Ok(StatusCode::NO_CONTENT)
}

fn site_from_row(row: sqlx::postgres::PgRow) -> Result<Site, SiteDomainError> {
    Ok(Site {
        id: row.get("id"),
        project_id: row.get("project_id"),
        service_id: row.get("service_id"),
        repository_id: row.get("repository_id"),
        environment_id: row.get("environment_id"),
        name: row.get("name"),
        description: row.get("description"),
        framework: row.get("framework"),
        canonical_url: row.get("canonical_url"),
        lifecycle_status: row.get("lifecycle_status"),
        health_status: row.get("health_status"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}

fn domain_from_row(row: sqlx::postgres::PgRow) -> Result<Domain, SiteDomainError> {
    Ok(Domain {
        id: row.get("id"),
        project_id: row.get("project_id"),
        site_id: row.get("site_id"),
        hostname: row.get("hostname"),
        registrar: row.get("registrar"),
        dns_provider: row.get("dns_provider"),
        routing_mode: row.get("routing_mode"),
        is_primary: row.get("is_primary"),
        expires_at: row.get("expires_at"),
        tls_status: row.get("tls_status"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}

fn normalize_hostname(value: &str) -> Result<String, SiteDomainError> {
    let hostname = value.trim().trim_end_matches('.').to_ascii_lowercase();
    if hostname.is_empty() || hostname.len() > 253 || !hostname.contains('.') {
        return Err(SiteDomainError::Invalid);
    }
    for label in hostname.split('.') {
        if label.is_empty()
            || label.len() > 63
            || label.starts_with('-')
            || label.ends_with('-')
            || !label
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        {
            return Err(SiteDomainError::Invalid);
        }
    }
    Ok(hostname)
}

fn normalize_http_url(value: Option<String>) -> Result<Option<String>, SiteDomainError> {
    let Some(value) = optional_value(value, 2048)? else {
        return Ok(None);
    };
    let url = Url::parse(&value).map_err(|_| SiteDomainError::Invalid)?;
    if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
        return Err(SiteDomainError::Invalid);
    }
    Ok(Some(url.to_string()))
}

fn normalize_expiry(value: Option<String>) -> Result<Option<DateTime<Utc>>, SiteDomainError> {
    let Some(value) = optional_value(value, 64)? else {
        return Ok(None);
    };
    if let Ok(value) = DateTime::parse_from_rfc3339(&value) {
        return Ok(Some(value.with_timezone(&Utc)));
    }
    let date =
        NaiveDate::parse_from_str(&value, "%Y-%m-%d").map_err(|_| SiteDomainError::Invalid)?;
    let datetime = date
        .and_hms_opt(23, 59, 59)
        .ok_or(SiteDomainError::Invalid)?;
    Ok(Some(Utc.from_utc_datetime(&datetime)))
}

fn required_text(value: &str, min: usize, max: usize) -> Result<String, SiteDomainError> {
    let value = value.trim();
    if value.len() < min || value.len() > max {
        Err(SiteDomainError::Invalid)
    } else {
        Ok(value.to_string())
    }
}

fn optional_text(value: &str, max: usize) -> Result<String, SiteDomainError> {
    let value = value.trim();
    if value.len() > max {
        Err(SiteDomainError::Invalid)
    } else {
        Ok(value.to_string())
    }
}

fn optional_value(value: Option<String>, max: usize) -> Result<Option<String>, SiteDomainError> {
    match value.map(|value| value.trim().to_string()) {
        Some(value) if value.is_empty() => Ok(None),
        Some(value) if value.len() <= max => Ok(Some(value)),
        Some(_) => Err(SiteDomainError::Invalid),
        None => Ok(None),
    }
}

fn map_write_error(error: sqlx::Error) -> Result<Domain, SiteDomainError> {
    if matches!(&error, sqlx::Error::Database(database) if database.code().as_deref() == Some("23505"))
    {
        Err(SiteDomainError::Conflict)
    } else {
        Err(SiteDomainError::Sql(error))
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

fn map_sites_domains(error: SiteDomainError) -> ApiError {
    match error {
        SiteDomainError::NotFound => api_error(
            StatusCode::NOT_FOUND,
            "SITE_DOMAIN_NOT_FOUND",
            "site or domain not found",
        ),
        SiteDomainError::Invalid => api_error(
            StatusCode::BAD_REQUEST,
            "INVALID_REQUEST",
            "invalid site or domain request",
        ),
        SiteDomainError::Conflict => api_error(
            StatusCode::CONFLICT,
            "OPERATION_CONFLICT",
            "site or domain conflicts with an existing resource",
        ),
        other => {
            tracing::error!(error=%other, "site/domain storage error");
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
    fn hostname_is_normalized_and_validated() {
        assert_eq!(normalize_hostname("Example.COM.").unwrap(), "example.com");
        assert!(normalize_hostname("localhost").is_err());
        assert!(normalize_hostname("-bad.example.com").is_err());
        assert!(normalize_hostname("bad_.example.com").is_err());
    }

    #[test]
    fn canonical_url_requires_http() {
        assert!(normalize_http_url(Some("https://example.com".into())).is_ok());
        assert!(normalize_http_url(Some("ssh://example.com".into())).is_err());
    }

    #[test]
    fn expiry_accepts_date_or_rfc3339() {
        assert!(normalize_expiry(Some("2027-01-31".into())).is_ok());
        assert!(normalize_expiry(Some("2027-01-31T12:00:00Z".into())).is_ok());
        assert!(normalize_expiry(Some("next year".into())).is_err());
    }
}
