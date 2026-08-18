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

#[derive(Debug, Clone)]
pub struct StatusPageStore {
    pool: PgPool,
}

#[derive(Debug, Clone, Serialize)]
pub struct StatusPageComponent {
    pub id: Uuid,
    pub resource_type: String,
    pub resource_id: Uuid,
    pub display_name: String,
    pub internal_status: String,
    pub public_status: String,
    pub sort_order: i32,
}

#[derive(Debug, Clone, Serialize)]
pub struct StatusIncidentPublication {
    pub id: Uuid,
    pub incident_id: Uuid,
    pub incident_status: String,
    pub incident_severity: String,
    pub public_title: String,
    pub public_message: String,
    pub is_published: bool,
    pub published_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
pub struct StatusPageView {
    pub id: Uuid,
    pub project_id: Uuid,
    pub name: String,
    pub slug: String,
    pub visibility: String,
    pub overall_status: String,
    pub components: Vec<StatusPageComponent>,
    pub incident_publications: Vec<StatusIncidentPublication>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PublicStatusComponent {
    pub name: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct PublicStatusIncident {
    pub title: String,
    pub message: String,
    pub status: String,
    pub started_at: DateTime<Utc>,
    pub resolved_at: Option<DateTime<Utc>>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PublicStatusPage {
    pub name: String,
    pub overall_status: String,
    pub components: Vec<PublicStatusComponent>,
    pub incidents: Vec<PublicStatusIncident>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateStatusPageRequest {
    pub name: String,
    pub slug: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdateStatusPageRequest {
    pub name: String,
    pub slug: String,
    pub visibility: String,
}

#[derive(Debug, Deserialize)]
pub struct AddStatusComponentRequest {
    pub resource_type: String,
    pub resource_id: Uuid,
    pub display_name: String,
    #[serde(default = "default_sort_order")]
    pub sort_order: i32,
}

#[derive(Debug, Deserialize)]
pub struct PublishIncidentRequest {
    pub incident_id: Uuid,
    pub public_title: String,
    pub public_message: String,
    #[serde(default = "default_true")]
    pub is_published: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum StatusPageError {
    #[error("status page resource not found")]
    NotFound,
    #[error("invalid status page request")]
    Invalid,
    #[error("status page conflict")]
    Conflict,
    #[error(transparent)]
    Sql(#[from] sqlx::Error),
}

impl StatusPageStore {
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
    ) -> Result<(), StatusPageError> {
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
            Err(StatusPageError::NotFound)
        }
    }

    pub async fn list(
        &self,
        organization_id: Uuid,
        project_id: Uuid,
    ) -> Result<Vec<StatusPageView>, StatusPageError> {
        self.authorize_project(organization_id, project_id).await?;
        let ids: Vec<Uuid> = sqlx::query_scalar(
            "SELECT id FROM status_pages WHERE organization_id=$1 AND project_id=$2 ORDER BY created_at",
        )
        .bind(organization_id)
        .bind(project_id)
        .fetch_all(&self.pool)
        .await?;
        let mut result = Vec::with_capacity(ids.len());
        for id in ids {
            result.push(self.get(organization_id, project_id, id).await?);
        }
        Ok(result)
    }

    pub async fn get(
        &self,
        organization_id: Uuid,
        project_id: Uuid,
        page_id: Uuid,
    ) -> Result<StatusPageView, StatusPageError> {
        let row = sqlx::query(
            "SELECT id,project_id,name,slug,visibility,created_at,updated_at FROM status_pages WHERE id=$1 AND organization_id=$2 AND project_id=$3",
        )
        .bind(page_id)
        .bind(organization_id)
        .bind(project_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(StatusPageError::NotFound)?;
        let components = self
            .components(organization_id, project_id, page_id)
            .await?;
        let publications = self
            .publications(organization_id, project_id, page_id)
            .await?;
        let overall_status = overall_status(&components, &publications);
        Ok(StatusPageView {
            id: row.get("id"),
            project_id: row.get("project_id"),
            name: row.get("name"),
            slug: row.get("slug"),
            visibility: row.get("visibility"),
            overall_status,
            components,
            incident_publications: publications,
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        })
    }

    pub async fn create(
        &self,
        identity: crate::persistence::WebIdentity,
        project_id: Uuid,
        request: CreateStatusPageRequest,
    ) -> Result<StatusPageView, StatusPageError> {
        self.authorize_project(identity.organization_id, project_id)
            .await?;
        let name = required_text(&request.name, 1, 160)?;
        let slug = normalize_slug(&request.slug)?;
        let id = Uuid::new_v4();
        let mut tx = self.pool.begin().await?;
        let insert = sqlx::query(
            "INSERT INTO status_pages(id,organization_id,project_id,name,slug,visibility,created_by,created_at,updated_at) VALUES($1,$2,$3,$4,$5,'INTERNAL',$6,NOW(),NOW())",
        )
        .bind(id)
        .bind(identity.organization_id)
        .bind(project_id)
        .bind(&name)
        .bind(&slug)
        .bind(identity.user_id)
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
            "status_page.created",
            serde_json::json!({"status_page_id":id,"slug":slug}),
        )
        .await?;
        tx.commit().await?;
        self.get(identity.organization_id, project_id, id).await
    }

    pub async fn update(
        &self,
        identity: crate::persistence::WebIdentity,
        project_id: Uuid,
        page_id: Uuid,
        request: UpdateStatusPageRequest,
    ) -> Result<StatusPageView, StatusPageError> {
        self.get(identity.organization_id, project_id, page_id)
            .await?;
        let name = required_text(&request.name, 1, 160)?;
        let slug = normalize_slug(&request.slug)?;
        let visibility = request.visibility.trim().to_uppercase();
        if !matches!(visibility.as_str(), "INTERNAL" | "PUBLIC") {
            return Err(StatusPageError::Invalid);
        }
        let mut tx = self.pool.begin().await?;
        let update = sqlx::query(
            "UPDATE status_pages SET name=$1,slug=$2,visibility=$3,updated_at=NOW() WHERE id=$4 AND organization_id=$5 AND project_id=$6",
        )
        .bind(&name)
        .bind(&slug)
        .bind(&visibility)
        .bind(page_id)
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
            "status_page.updated",
            serde_json::json!({"status_page_id":page_id,"slug":slug,"visibility":visibility}),
        )
        .await?;
        tx.commit().await?;
        self.get(identity.organization_id, project_id, page_id)
            .await
    }

    pub async fn delete(
        &self,
        identity: crate::persistence::WebIdentity,
        project_id: Uuid,
        page_id: Uuid,
    ) -> Result<(), StatusPageError> {
        let page = self
            .get(identity.organization_id, project_id, page_id)
            .await?;
        let mut tx = self.pool.begin().await?;
        sqlx::query(
            "DELETE FROM status_pages WHERE id=$1 AND organization_id=$2 AND project_id=$3",
        )
        .bind(page_id)
        .bind(identity.organization_id)
        .bind(project_id)
        .execute(&mut *tx)
        .await?;
        touch_project(&mut tx, identity.organization_id, project_id).await?;
        audit_event(
            &mut tx,
            identity,
            project_id,
            "status_page.deleted",
            serde_json::json!({"status_page_id":page_id,"slug":page.slug}),
        )
        .await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn add_component(
        &self,
        identity: crate::persistence::WebIdentity,
        project_id: Uuid,
        page_id: Uuid,
        request: AddStatusComponentRequest,
    ) -> Result<StatusPageView, StatusPageError> {
        self.get(identity.organization_id, project_id, page_id)
            .await?;
        let resource_type = request.resource_type.trim().to_uppercase();
        let display_name = required_text(&request.display_name, 1, 160)?;
        let (site_id, service_id) = match resource_type.as_str() {
            "SITE" => {
                ensure_project_resource(
                    &self.pool,
                    "sites",
                    identity.organization_id,
                    project_id,
                    request.resource_id,
                )
                .await?;
                (Some(request.resource_id), None)
            }
            "SERVICE" => {
                ensure_project_resource(
                    &self.pool,
                    "services",
                    identity.organization_id,
                    project_id,
                    request.resource_id,
                )
                .await?;
                (None, Some(request.resource_id))
            }
            _ => return Err(StatusPageError::Invalid),
        };
        let id = Uuid::new_v4();
        let mut tx = self.pool.begin().await?;
        let insert = sqlx::query(
            "INSERT INTO status_page_components(id,organization_id,project_id,status_page_id,site_id,service_id,display_name,sort_order,created_at) VALUES($1,$2,$3,$4,$5,$6,$7,$8,NOW())",
        )
        .bind(id)
        .bind(identity.organization_id)
        .bind(project_id)
        .bind(page_id)
        .bind(site_id)
        .bind(service_id)
        .bind(&display_name)
        .bind(request.sort_order)
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
            "status_page.component_added",
            serde_json::json!({"status_page_id":page_id,"component_id":id,"resource_type":resource_type,"resource_id":request.resource_id}),
        )
        .await?;
        tx.commit().await?;
        self.get(identity.organization_id, project_id, page_id)
            .await
    }

    pub async fn remove_component(
        &self,
        identity: crate::persistence::WebIdentity,
        project_id: Uuid,
        page_id: Uuid,
        component_id: Uuid,
    ) -> Result<StatusPageView, StatusPageError> {
        self.get(identity.organization_id, project_id, page_id)
            .await?;
        let exists: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM status_page_components WHERE id=$1 AND organization_id=$2 AND project_id=$3 AND status_page_id=$4)",
        )
        .bind(component_id)
        .bind(identity.organization_id)
        .bind(project_id)
        .bind(page_id)
        .fetch_one(&self.pool)
        .await?;
        if !exists {
            return Err(StatusPageError::NotFound);
        }
        let mut tx = self.pool.begin().await?;
        sqlx::query(
            "DELETE FROM status_page_components WHERE id=$1 AND organization_id=$2 AND project_id=$3 AND status_page_id=$4",
        )
        .bind(component_id)
        .bind(identity.organization_id)
        .bind(project_id)
        .bind(page_id)
        .execute(&mut *tx)
        .await?;
        touch_project(&mut tx, identity.organization_id, project_id).await?;
        audit_event(
            &mut tx,
            identity,
            project_id,
            "status_page.component_removed",
            serde_json::json!({"status_page_id":page_id,"component_id":component_id}),
        )
        .await?;
        tx.commit().await?;
        self.get(identity.organization_id, project_id, page_id)
            .await
    }

    pub async fn publish_incident(
        &self,
        identity: crate::persistence::WebIdentity,
        project_id: Uuid,
        page_id: Uuid,
        request: PublishIncidentRequest,
    ) -> Result<StatusPageView, StatusPageError> {
        self.get(identity.organization_id, project_id, page_id)
            .await?;
        let incident_exists: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM incidents WHERE id=$1 AND organization_id=$2 AND project_id=$3)",
        )
        .bind(request.incident_id)
        .bind(identity.organization_id)
        .bind(project_id)
        .fetch_one(&self.pool)
        .await?;
        if !incident_exists {
            return Err(StatusPageError::Invalid);
        }
        let public_title = required_text(&request.public_title, 1, 200)?;
        let public_message = required_text(&request.public_message, 1, 4000)?;
        let id = Uuid::new_v4();
        let mut tx = self.pool.begin().await?;
        sqlx::query(
            "INSERT INTO status_page_incidents(id,organization_id,project_id,status_page_id,incident_id,public_title,public_message,is_published,published_at,updated_at) VALUES($1,$2,$3,$4,$5,$6,$7,$8,NOW(),NOW()) ON CONFLICT(status_page_id,incident_id) DO UPDATE SET public_title=EXCLUDED.public_title,public_message=EXCLUDED.public_message,is_published=EXCLUDED.is_published,updated_at=NOW()",
        )
        .bind(id)
        .bind(identity.organization_id)
        .bind(project_id)
        .bind(page_id)
        .bind(request.incident_id)
        .bind(&public_title)
        .bind(&public_message)
        .bind(request.is_published)
        .execute(&mut *tx)
        .await?;
        touch_project(&mut tx, identity.organization_id, project_id).await?;
        audit_event(
            &mut tx,
            identity,
            project_id,
            "status_page.incident_publication_updated",
            serde_json::json!({"status_page_id":page_id,"incident_id":request.incident_id,"is_published":request.is_published}),
        )
        .await?;
        tx.commit().await?;
        self.get(identity.organization_id, project_id, page_id)
            .await
    }

    pub async fn public(&self, slug: &str) -> Result<PublicStatusPage, StatusPageError> {
        let slug = normalize_slug(slug)?;
        let row = sqlx::query(
            "SELECT id,organization_id,project_id,name,updated_at FROM status_pages WHERE slug=$1 AND visibility='PUBLIC'",
        )
        .bind(&slug)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(StatusPageError::NotFound)?;
        let page_id: Uuid = row.get("id");
        let organization_id: Uuid = row.get("organization_id");
        let project_id: Uuid = row.get("project_id");
        let components = self
            .components(organization_id, project_id, page_id)
            .await?;
        let publications = self
            .publications(organization_id, project_id, page_id)
            .await?;
        let overall = overall_status(&components, &publications);
        let incident_rows = sqlx::query(
            "SELECT spi.public_title,spi.public_message,i.status,i.started_at,i.resolved_at,spi.updated_at FROM status_page_incidents spi JOIN incidents i ON i.id=spi.incident_id WHERE spi.status_page_id=$1 AND spi.is_published=TRUE ORDER BY CASE WHEN i.status='RESOLVED' THEN 1 ELSE 0 END,i.started_at DESC LIMIT 50",
        )
        .bind(page_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(PublicStatusPage {
            name: row.get("name"),
            overall_status: overall,
            components: components
                .into_iter()
                .map(|component| PublicStatusComponent {
                    name: component.display_name,
                    status: component.public_status,
                })
                .collect(),
            incidents: incident_rows
                .into_iter()
                .map(|incident| PublicStatusIncident {
                    title: incident.get("public_title"),
                    message: incident.get("public_message"),
                    status: incident.get("status"),
                    started_at: incident.get("started_at"),
                    resolved_at: incident.get("resolved_at"),
                    updated_at: incident.get("updated_at"),
                })
                .collect(),
            updated_at: row.get("updated_at"),
        })
    }

    async fn components(
        &self,
        organization_id: Uuid,
        project_id: Uuid,
        page_id: Uuid,
    ) -> Result<Vec<StatusPageComponent>, StatusPageError> {
        let rows = sqlx::query(
            "SELECT c.id,c.site_id,c.service_id,c.display_name,c.sort_order,s.health_status AS site_status,svc.status AS service_status FROM status_page_components c LEFT JOIN sites s ON s.id=c.site_id LEFT JOIN services svc ON svc.id=c.service_id WHERE c.organization_id=$1 AND c.project_id=$2 AND c.status_page_id=$3 ORDER BY c.sort_order,c.display_name",
        )
        .bind(organization_id)
        .bind(project_id)
        .bind(page_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(|row| {
                let site_id: Option<Uuid> = row.get("site_id");
                let (resource_type, resource_id, internal_status) = match site_id {
                    Some(id) => (
                        "SITE".to_string(),
                        id,
                        row.get::<Option<String>, _>("site_status")
                            .unwrap_or_else(|| "UNKNOWN".into()),
                    ),
                    None => (
                        "SERVICE".to_string(),
                        row.get::<Uuid, _>("service_id"),
                        row.get::<Option<String>, _>("service_status")
                            .unwrap_or_else(|| "UNKNOWN".into()),
                    ),
                };
                let public_status = component_public_status(&internal_status).to_string();
                StatusPageComponent {
                    id: row.get("id"),
                    resource_type,
                    resource_id,
                    display_name: row.get("display_name"),
                    internal_status,
                    public_status,
                    sort_order: row.get("sort_order"),
                }
            })
            .collect())
    }

    async fn publications(
        &self,
        organization_id: Uuid,
        project_id: Uuid,
        page_id: Uuid,
    ) -> Result<Vec<StatusIncidentPublication>, StatusPageError> {
        let rows = sqlx::query(
            "SELECT spi.id,spi.incident_id,i.status,i.severity,spi.public_title,spi.public_message,spi.is_published,spi.published_at,spi.updated_at FROM status_page_incidents spi JOIN incidents i ON i.id=spi.incident_id WHERE spi.organization_id=$1 AND spi.project_id=$2 AND spi.status_page_id=$3 ORDER BY spi.published_at DESC",
        )
        .bind(organization_id)
        .bind(project_id)
        .bind(page_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(|row| StatusIncidentPublication {
                id: row.get("id"),
                incident_id: row.get("incident_id"),
                incident_status: row.get("status"),
                incident_severity: row.get("severity"),
                public_title: row.get("public_title"),
                public_message: row.get("public_message"),
                is_published: row.get("is_published"),
                published_at: row.get("published_at"),
                updated_at: row.get("updated_at"),
            })
            .collect())
    }
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/projects/:project_id/status-pages",
            get(list_status_pages).post(create_status_page),
        )
        .route(
            "/projects/:project_id/status-pages/:page_id",
            axum::routing::put(update_status_page).delete(delete_status_page),
        )
        .route(
            "/projects/:project_id/status-pages/:page_id/components",
            post(add_status_component),
        )
        .route(
            "/projects/:project_id/status-pages/:page_id/components/:component_id",
            axum::routing::delete(remove_status_component),
        )
        .route(
            "/projects/:project_id/status-pages/:page_id/incidents",
            post(publish_status_incident),
        )
        .route("/public/status/:slug", get(get_public_status_page))
}

async fn list_status_pages(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(project_id): Path<Uuid>,
) -> Result<Json<Vec<StatusPageView>>, ApiError> {
    let identity = web_identity(&state, &headers).await?;
    Ok(Json(
        state
            .status_pages
            .list(identity.organization_id, project_id)
            .await
            .map_err(map_status_page)?,
    ))
}

async fn create_status_page(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(project_id): Path<Uuid>,
    Json(request): Json<CreateStatusPageRequest>,
) -> Result<(StatusCode, Json<StatusPageView>), ApiError> {
    let identity = web_identity(&state, &headers).await?;
    let page = state
        .status_pages
        .create(identity, project_id, request)
        .await
        .map_err(map_status_page)?;
    Ok((StatusCode::CREATED, Json(page)))
}

async fn update_status_page(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((project_id, page_id)): Path<(Uuid, Uuid)>,
    Json(request): Json<UpdateStatusPageRequest>,
) -> Result<Json<StatusPageView>, ApiError> {
    let identity = web_identity(&state, &headers).await?;
    Ok(Json(
        state
            .status_pages
            .update(identity, project_id, page_id, request)
            .await
            .map_err(map_status_page)?,
    ))
}

async fn delete_status_page(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((project_id, page_id)): Path<(Uuid, Uuid)>,
) -> Result<StatusCode, ApiError> {
    let identity = web_identity(&state, &headers).await?;
    state
        .status_pages
        .delete(identity, project_id, page_id)
        .await
        .map_err(map_status_page)?;
    Ok(StatusCode::NO_CONTENT)
}

async fn add_status_component(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((project_id, page_id)): Path<(Uuid, Uuid)>,
    Json(request): Json<AddStatusComponentRequest>,
) -> Result<Json<StatusPageView>, ApiError> {
    let identity = web_identity(&state, &headers).await?;
    Ok(Json(
        state
            .status_pages
            .add_component(identity, project_id, page_id, request)
            .await
            .map_err(map_status_page)?,
    ))
}

async fn remove_status_component(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((project_id, page_id, component_id)): Path<(Uuid, Uuid, Uuid)>,
) -> Result<Json<StatusPageView>, ApiError> {
    let identity = web_identity(&state, &headers).await?;
    Ok(Json(
        state
            .status_pages
            .remove_component(identity, project_id, page_id, component_id)
            .await
            .map_err(map_status_page)?,
    ))
}

async fn publish_status_incident(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((project_id, page_id)): Path<(Uuid, Uuid)>,
    Json(request): Json<PublishIncidentRequest>,
) -> Result<Json<StatusPageView>, ApiError> {
    let identity = web_identity(&state, &headers).await?;
    Ok(Json(
        state
            .status_pages
            .publish_incident(identity, project_id, page_id, request)
            .await
            .map_err(map_status_page)?,
    ))
}

async fn get_public_status_page(
    State(state): State<AppState>,
    Path(slug): Path<String>,
) -> Result<Json<PublicStatusPage>, ApiError> {
    Ok(Json(
        state
            .status_pages
            .public(&slug)
            .await
            .map_err(map_status_page)?,
    ))
}

fn component_public_status(value: &str) -> &'static str {
    match value.trim().to_uppercase().as_str() {
        "HEALTHY" | "RUNNING" | "ACTIVE" | "OPERATIONAL" => "OPERATIONAL",
        "DEGRADED" => "DEGRADED",
        "DOWN" | "ERROR" | "FAILED" | "STOPPED" | "UNHEALTHY" => "OUTAGE",
        _ => "UNKNOWN",
    }
}

fn overall_status(
    components: &[StatusPageComponent],
    publications: &[StatusIncidentPublication],
) -> String {
    let mut incident_level = 0_u8;
    for publication in publications
        .iter()
        .filter(|publication| publication.is_published && publication.incident_status != "RESOLVED")
    {
        incident_level = incident_level.max(match publication.incident_severity.as_str() {
            "CRITICAL" => 4,
            "MAJOR" => 3,
            "MINOR" => 2,
            _ => 0,
        });
    }
    if incident_level >= 4 {
        return "MAJOR_OUTAGE".into();
    }
    if incident_level == 3 {
        return "PARTIAL_OUTAGE".into();
    }
    if incident_level == 2 {
        return "DEGRADED".into();
    }
    if components
        .iter()
        .any(|component| component.public_status == "OUTAGE")
    {
        return "PARTIAL_OUTAGE".into();
    }
    if components
        .iter()
        .any(|component| component.public_status == "DEGRADED")
    {
        return "DEGRADED".into();
    }
    if components.is_empty()
        || components
            .iter()
            .all(|component| component.public_status == "UNKNOWN")
    {
        return "UNKNOWN".into();
    }
    "OPERATIONAL".into()
}

fn normalize_slug(value: &str) -> Result<String, StatusPageError> {
    let value = value.trim().to_ascii_lowercase();
    if value.len() < 3
        || value.len() > 80
        || value.starts_with('-')
        || value.ends_with('-')
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
    {
        return Err(StatusPageError::Invalid);
    }
    Ok(value)
}

fn required_text(value: &str, min: usize, max: usize) -> Result<String, StatusPageError> {
    let value = value.trim();
    if value.len() < min || value.len() > max {
        Err(StatusPageError::Invalid)
    } else {
        Ok(value.to_string())
    }
}

async fn ensure_project_resource(
    pool: &PgPool,
    table: &str,
    organization_id: Uuid,
    project_id: Uuid,
    resource_id: Uuid,
) -> Result<(), StatusPageError> {
    let sql = match table {
        "sites" => {
            "SELECT EXISTS(SELECT 1 FROM sites WHERE id=$1 AND organization_id=$2 AND project_id=$3)"
        }
        "services" => {
            "SELECT EXISTS(SELECT 1 FROM services WHERE id=$1 AND organization_id=$2 AND project_id=$3)"
        }
        _ => return Err(StatusPageError::Invalid),
    };
    let exists: bool = sqlx::query_scalar(sql)
        .bind(resource_id)
        .bind(organization_id)
        .bind(project_id)
        .fetch_one(pool)
        .await?;
    if exists {
        Ok(())
    } else {
        Err(StatusPageError::Invalid)
    }
}

fn map_write_error(error: sqlx::Error) -> Result<StatusPageView, StatusPageError> {
    if matches!(&error, sqlx::Error::Database(database) if database.code().as_deref() == Some("23505"))
    {
        Err(StatusPageError::Conflict)
    } else {
        Err(StatusPageError::Sql(error))
    }
}

fn default_sort_order() -> i32 {
    100
}
fn default_true() -> bool {
    true
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

fn map_status_page(error: StatusPageError) -> ApiError {
    match error {
        StatusPageError::NotFound => api_error(
            StatusCode::NOT_FOUND,
            "STATUS_PAGE_NOT_FOUND",
            "status page resource not found",
        ),
        StatusPageError::Invalid => api_error(
            StatusCode::BAD_REQUEST,
            "INVALID_REQUEST",
            "invalid status page request",
        ),
        StatusPageError::Conflict => api_error(
            StatusCode::CONFLICT,
            "OPERATION_CONFLICT",
            "status page slug or component already exists",
        ),
        other => {
            tracing::error!(error=%other, "status page storage error");
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
    fn slug_is_public_url_safe() {
        assert_eq!(normalize_slug("Argus-Status").unwrap(), "argus-status");
        assert!(normalize_slug("ab").is_err());
        assert!(normalize_slug("bad slug").is_err());
        assert!(normalize_slug("-bad").is_err());
    }

    #[test]
    fn public_component_status_is_coarse() {
        assert_eq!(component_public_status("HEALTHY"), "OPERATIONAL");
        assert_eq!(component_public_status("FAILED"), "OUTAGE");
        assert_eq!(component_public_status("something-internal"), "UNKNOWN");
    }

    #[test]
    fn unpublished_incidents_do_not_change_overall_status() {
        let publication = StatusIncidentPublication {
            id: Uuid::new_v4(),
            incident_id: Uuid::new_v4(),
            incident_status: "INVESTIGATING".into(),
            incident_severity: "CRITICAL".into(),
            public_title: "x".into(),
            public_message: "x".into(),
            is_published: false,
            published_at: Utc::now(),
            updated_at: Utc::now(),
        };
        assert_eq!(overall_status(&[], &[publication]), "UNKNOWN");
    }
}
