use crate::{ApiError, AppState, api_error, web_identity};
use axum::{
    Json, Router,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    routing::get,
};
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};
use std::collections::{HashMap, HashSet, VecDeque};
use uuid::Uuid;

const RESOURCE_TYPES: &[&str] = &[
    "SERVICE",
    "SITE",
    "DOMAIN",
    "SERVER",
    "ENVIRONMENT",
    "REPOSITORY",
];

#[derive(Debug, Clone)]
pub struct DependencyGraphStore {
    pool: PgPool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ResourceNode {
    pub resource_type: String,
    pub resource_id: Uuid,
    pub name: String,
    pub status: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DependencyEdge {
    pub id: Option<Uuid>,
    pub source_type: String,
    pub source_id: Uuid,
    pub target_type: String,
    pub target_id: Uuid,
    pub relationship: String,
    pub origin: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct DependencyGraph {
    pub nodes: Vec<ResourceNode>,
    pub edges: Vec<DependencyEdge>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ResourceRef {
    pub resource_type: String,
    pub resource_id: Uuid,
    pub name: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ImpactedResource {
    pub resource: ResourceRef,
    pub distance: u32,
    pub path: Vec<ResourceRef>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ImpactView {
    pub root: ResourceRef,
    pub affected_count: usize,
    pub affected: Vec<ImpactedResource>,
}

#[derive(Debug, Deserialize)]
pub struct CreateDependencyRequest {
    pub source_type: String,
    pub source_id: Uuid,
    pub target_type: String,
    pub target_id: Uuid,
    #[serde(default = "default_relationship")]
    pub relationship: String,
}

#[derive(Debug, thiserror::Error)]
pub enum DependencyGraphError {
    #[error("resource or dependency not found")]
    NotFound,
    #[error("invalid dependency request")]
    Invalid,
    #[error("dependency already exists")]
    Conflict,
    #[error(transparent)]
    Sql(#[from] sqlx::Error),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct NodeKey {
    resource_type: String,
    resource_id: Uuid,
}

impl NodeKey {
    fn new(resource_type: impl Into<String>, resource_id: Uuid) -> Self {
        Self {
            resource_type: resource_type.into(),
            resource_id,
        }
    }
}

impl DependencyGraphStore {
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
    ) -> Result<(), DependencyGraphError> {
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
            Err(DependencyGraphError::NotFound)
        }
    }

    pub async fn graph(
        &self,
        organization_id: Uuid,
        project_id: Uuid,
    ) -> Result<DependencyGraph, DependencyGraphError> {
        self.authorize_project(organization_id, project_id).await?;
        self.load_graph(organization_id, project_id).await
    }

    pub async fn create_dependency(
        &self,
        identity: crate::persistence::WebIdentity,
        project_id: Uuid,
        request: CreateDependencyRequest,
    ) -> Result<DependencyEdge, DependencyGraphError> {
        self.authorize_project(identity.organization_id, project_id)
            .await?;
        let source_type = normalize_resource_type(&request.source_type)?;
        let target_type = normalize_resource_type(&request.target_type)?;
        let relationship = request.relationship.trim().to_uppercase();
        if !matches!(relationship.as_str(), "DEPENDS_ON" | "USES")
            || (source_type == target_type && request.source_id == request.target_id)
        {
            return Err(DependencyGraphError::Invalid);
        }
        let graph = self
            .load_graph(identity.organization_id, project_id)
            .await?;
        let node_keys: HashSet<NodeKey> = graph
            .nodes
            .iter()
            .map(|node| NodeKey::new(&node.resource_type, node.resource_id))
            .collect();
        if !node_keys.contains(&NodeKey::new(&source_type, request.source_id))
            || !node_keys.contains(&NodeKey::new(&target_type, request.target_id))
        {
            return Err(DependencyGraphError::Invalid);
        }

        let id = Uuid::new_v4();
        let mut tx = self.pool.begin().await?;
        let insert = sqlx::query(
            "INSERT INTO resource_dependencies(id,organization_id,project_id,source_type,source_id,target_type,target_id,relationship,created_by,created_at) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,NOW())",
        )
        .bind(id)
        .bind(identity.organization_id)
        .bind(project_id)
        .bind(&source_type)
        .bind(request.source_id)
        .bind(&target_type)
        .bind(request.target_id)
        .bind(&relationship)
        .bind(identity.user_id)
        .execute(&mut *tx)
        .await;
        if let Err(error) = insert {
            tx.rollback().await?;
            if is_unique_violation(&error) {
                return Err(DependencyGraphError::Conflict);
            }
            return Err(DependencyGraphError::Sql(error));
        }
        touch_project(&mut tx, identity.organization_id, project_id).await?;
        audit_event(
            &mut tx,
            identity,
            project_id,
            "dependency.created",
            serde_json::json!({
                "dependency_id":id,
                "source_type":source_type,
                "source_id":request.source_id,
                "target_type":target_type,
                "target_id":request.target_id,
                "relationship":relationship
            }),
        )
        .await?;
        tx.commit().await?;
        Ok(DependencyEdge {
            id: Some(id),
            source_type,
            source_id: request.source_id,
            target_type,
            target_id: request.target_id,
            relationship,
            origin: "MANUAL".into(),
        })
    }

    pub async fn delete_dependency(
        &self,
        identity: crate::persistence::WebIdentity,
        project_id: Uuid,
        dependency_id: Uuid,
    ) -> Result<(), DependencyGraphError> {
        self.authorize_project(identity.organization_id, project_id)
            .await?;
        let row = sqlx::query(
            "SELECT source_type,source_id,target_type,target_id,relationship FROM resource_dependencies WHERE id=$1 AND organization_id=$2 AND project_id=$3",
        )
        .bind(dependency_id)
        .bind(identity.organization_id)
        .bind(project_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(DependencyGraphError::NotFound)?;
        let mut tx = self.pool.begin().await?;
        sqlx::query(
            "DELETE FROM resource_dependencies WHERE id=$1 AND organization_id=$2 AND project_id=$3",
        )
        .bind(dependency_id)
        .bind(identity.organization_id)
        .bind(project_id)
        .execute(&mut *tx)
        .await?;
        touch_project(&mut tx, identity.organization_id, project_id).await?;
        audit_event(
            &mut tx,
            identity,
            project_id,
            "dependency.deleted",
            serde_json::json!({
                "dependency_id":dependency_id,
                "source_type":row.get::<String,_>("source_type"),
                "source_id":row.get::<Uuid,_>("source_id"),
                "target_type":row.get::<String,_>("target_type"),
                "target_id":row.get::<Uuid,_>("target_id"),
                "relationship":row.get::<String,_>("relationship")
            }),
        )
        .await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn impact(
        &self,
        organization_id: Uuid,
        project_id: Uuid,
        resource_type: &str,
        resource_id: Uuid,
    ) -> Result<ImpactView, DependencyGraphError> {
        self.authorize_project(organization_id, project_id).await?;
        let resource_type = normalize_resource_type(resource_type)?;
        let graph = self.load_graph(organization_id, project_id).await?;
        let nodes: HashMap<NodeKey, ResourceNode> = graph
            .nodes
            .into_iter()
            .map(|node| (NodeKey::new(&node.resource_type, node.resource_id), node))
            .collect();
        let root_key = NodeKey::new(&resource_type, resource_id);
        let root_node = nodes.get(&root_key).ok_or(DependencyGraphError::NotFound)?;
        let root = resource_ref(root_node);

        let mut reverse: HashMap<NodeKey, Vec<NodeKey>> = HashMap::new();
        for edge in graph.edges {
            let source = NodeKey::new(edge.source_type, edge.source_id);
            let target = NodeKey::new(edge.target_type, edge.target_id);
            if nodes.contains_key(&source) && nodes.contains_key(&target) {
                reverse.entry(target).or_default().push(source);
            }
        }

        let mut visited = HashSet::new();
        visited.insert(root_key.clone());
        let mut queue = VecDeque::new();
        queue.push_back((root_key, 0_u32, vec![root.clone()]));
        let mut affected = Vec::new();
        while let Some((current, distance, path)) = queue.pop_front() {
            let Some(dependents) = reverse.get(&current) else {
                continue;
            };
            for dependent in dependents {
                if !visited.insert(dependent.clone()) {
                    continue;
                }
                let Some(node) = nodes.get(dependent) else {
                    continue;
                };
                let resource = resource_ref(node);
                let mut dependent_path = path.clone();
                dependent_path.push(resource.clone());
                affected.push(ImpactedResource {
                    resource: resource.clone(),
                    distance: distance + 1,
                    path: dependent_path.clone(),
                });
                queue.push_back((dependent.clone(), distance + 1, dependent_path));
            }
        }
        affected.sort_by(|left, right| {
            left.distance
                .cmp(&right.distance)
                .then_with(|| {
                    left.resource
                        .resource_type
                        .cmp(&right.resource.resource_type)
                })
                .then_with(|| left.resource.name.cmp(&right.resource.name))
        });
        Ok(ImpactView {
            root,
            affected_count: affected.len(),
            affected,
        })
    }

    async fn load_graph(
        &self,
        organization_id: Uuid,
        project_id: Uuid,
    ) -> Result<DependencyGraph, DependencyGraphError> {
        let service_rows = sqlx::query(
            "SELECT id,name,status,server_id FROM services WHERE organization_id=$1 AND project_id=$2",
        )
        .bind(organization_id)
        .bind(project_id)
        .fetch_all(&self.pool)
        .await?;
        let site_rows = sqlx::query(
            "SELECT id,name,health_status,service_id FROM sites WHERE organization_id=$1 AND project_id=$2",
        )
        .bind(organization_id)
        .bind(project_id)
        .fetch_all(&self.pool)
        .await?;
        let domain_rows = sqlx::query(
            "SELECT id,hostname,tls_status,site_id FROM domains WHERE organization_id=$1 AND project_id=$2",
        )
        .bind(organization_id)
        .bind(project_id)
        .fetch_all(&self.pool)
        .await?;
        let server_rows = sqlx::query(
            "SELECT id,hostname FROM servers WHERE organization_id=$1 AND project_id=$2",
        )
        .bind(organization_id)
        .bind(project_id)
        .fetch_all(&self.pool)
        .await?;
        let environment_rows = sqlx::query(
            "SELECT id,name FROM environments WHERE organization_id=$1 AND project_id=$2",
        )
        .bind(organization_id)
        .bind(project_id)
        .fetch_all(&self.pool)
        .await?;
        let repository_rows = sqlx::query(
            "SELECT id,owner,name,sync_status FROM project_repositories WHERE organization_id=$1 AND project_id=$2",
        )
        .bind(organization_id)
        .bind(project_id)
        .fetch_all(&self.pool)
        .await?;

        let mut nodes = Vec::new();
        let mut edges = Vec::new();
        for row in service_rows {
            let id: Uuid = row.get("id");
            nodes.push(ResourceNode {
                resource_type: "SERVICE".into(),
                resource_id: id,
                name: row.get("name"),
                status: Some(row.get("status")),
            });
            if let Some(server_id) = row.get::<Option<Uuid>, _>("server_id") {
                edges.push(derived_edge(
                    "SERVICE",
                    id,
                    "SERVER",
                    server_id,
                    "HOSTED_ON",
                ));
            }
        }
        for row in site_rows {
            let id: Uuid = row.get("id");
            nodes.push(ResourceNode {
                resource_type: "SITE".into(),
                resource_id: id,
                name: row.get("name"),
                status: Some(row.get("health_status")),
            });
            if let Some(service_id) = row.get::<Option<Uuid>, _>("service_id") {
                edges.push(derived_edge("SITE", id, "SERVICE", service_id, "BACKED_BY"));
            }
        }
        for row in domain_rows {
            let id: Uuid = row.get("id");
            nodes.push(ResourceNode {
                resource_type: "DOMAIN".into(),
                resource_id: id,
                name: row.get("hostname"),
                status: Some(row.get("tls_status")),
            });
            if let Some(site_id) = row.get::<Option<Uuid>, _>("site_id") {
                edges.push(derived_edge("DOMAIN", id, "SITE", site_id, "ROUTES_TO"));
            }
        }
        for row in server_rows {
            nodes.push(ResourceNode {
                resource_type: "SERVER".into(),
                resource_id: row.get("id"),
                name: row.get("hostname"),
                status: None,
            });
        }
        for row in environment_rows {
            nodes.push(ResourceNode {
                resource_type: "ENVIRONMENT".into(),
                resource_id: row.get("id"),
                name: row.get("name"),
                status: None,
            });
        }
        for row in repository_rows {
            let owner: String = row.get("owner");
            let name: String = row.get("name");
            nodes.push(ResourceNode {
                resource_type: "REPOSITORY".into(),
                resource_id: row.get("id"),
                name: format!("{owner}/{name}"),
                status: Some(row.get("sync_status")),
            });
        }

        let manual_rows = sqlx::query(
            "SELECT id,source_type,source_id,target_type,target_id,relationship FROM resource_dependencies WHERE organization_id=$1 AND project_id=$2 ORDER BY created_at",
        )
        .bind(organization_id)
        .bind(project_id)
        .fetch_all(&self.pool)
        .await?;
        edges.extend(manual_rows.into_iter().map(|row| DependencyEdge {
            id: Some(row.get("id")),
            source_type: row.get("source_type"),
            source_id: row.get("source_id"),
            target_type: row.get("target_type"),
            target_id: row.get("target_id"),
            relationship: row.get("relationship"),
            origin: "MANUAL".into(),
        }));

        let node_keys: HashSet<NodeKey> = nodes
            .iter()
            .map(|node| NodeKey::new(&node.resource_type, node.resource_id))
            .collect();
        edges.retain(|edge| {
            node_keys.contains(&NodeKey::new(&edge.source_type, edge.source_id))
                && node_keys.contains(&NodeKey::new(&edge.target_type, edge.target_id))
        });
        nodes.sort_by(|left, right| {
            left.resource_type
                .cmp(&right.resource_type)
                .then_with(|| left.name.cmp(&right.name))
        });
        Ok(DependencyGraph { nodes, edges })
    }
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/projects/:project_id/dependency-graph",
            get(get_dependency_graph),
        )
        .route(
            "/projects/:project_id/dependencies",
            axum::routing::post(create_dependency),
        )
        .route(
            "/projects/:project_id/dependencies/:dependency_id",
            axum::routing::delete(delete_dependency),
        )
        .route(
            "/projects/:project_id/dependency-impact/:resource_type/:resource_id",
            get(get_dependency_impact),
        )
}

async fn get_dependency_graph(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(project_id): Path<Uuid>,
) -> Result<Json<DependencyGraph>, ApiError> {
    let identity = web_identity(&state, &headers).await?;
    Ok(Json(
        state
            .dependency_graph
            .graph(identity.organization_id, project_id)
            .await
            .map_err(map_dependency_graph)?,
    ))
}

async fn create_dependency(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(project_id): Path<Uuid>,
    Json(request): Json<CreateDependencyRequest>,
) -> Result<(StatusCode, Json<DependencyEdge>), ApiError> {
    let identity = web_identity(&state, &headers).await?;
    let edge = state
        .dependency_graph
        .create_dependency(identity, project_id, request)
        .await
        .map_err(map_dependency_graph)?;
    Ok((StatusCode::CREATED, Json(edge)))
}

async fn delete_dependency(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((project_id, dependency_id)): Path<(Uuid, Uuid)>,
) -> Result<StatusCode, ApiError> {
    let identity = web_identity(&state, &headers).await?;
    state
        .dependency_graph
        .delete_dependency(identity, project_id, dependency_id)
        .await
        .map_err(map_dependency_graph)?;
    Ok(StatusCode::NO_CONTENT)
}

async fn get_dependency_impact(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((project_id, resource_type, resource_id)): Path<(Uuid, String, Uuid)>,
) -> Result<Json<ImpactView>, ApiError> {
    let identity = web_identity(&state, &headers).await?;
    Ok(Json(
        state
            .dependency_graph
            .impact(
                identity.organization_id,
                project_id,
                &resource_type,
                resource_id,
            )
            .await
            .map_err(map_dependency_graph)?,
    ))
}

fn normalize_resource_type(value: &str) -> Result<String, DependencyGraphError> {
    let value = value.trim().to_uppercase();
    if RESOURCE_TYPES.contains(&value.as_str()) {
        Ok(value)
    } else {
        Err(DependencyGraphError::Invalid)
    }
}

fn derived_edge(
    source_type: &str,
    source_id: Uuid,
    target_type: &str,
    target_id: Uuid,
    relationship: &str,
) -> DependencyEdge {
    DependencyEdge {
        id: None,
        source_type: source_type.into(),
        source_id,
        target_type: target_type.into(),
        target_id,
        relationship: relationship.into(),
        origin: "DERIVED".into(),
    }
}

fn resource_ref(node: &ResourceNode) -> ResourceRef {
    ResourceRef {
        resource_type: node.resource_type.clone(),
        resource_id: node.resource_id,
        name: node.name.clone(),
    }
}

fn default_relationship() -> String {
    "DEPENDS_ON".into()
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

fn map_dependency_graph(error: DependencyGraphError) -> ApiError {
    match error {
        DependencyGraphError::NotFound => api_error(
            StatusCode::NOT_FOUND,
            "DEPENDENCY_RESOURCE_NOT_FOUND",
            "dependency resource not found",
        ),
        DependencyGraphError::Invalid => api_error(
            StatusCode::BAD_REQUEST,
            "INVALID_REQUEST",
            "invalid dependency request",
        ),
        DependencyGraphError::Conflict => api_error(
            StatusCode::CONFLICT,
            "OPERATION_CONFLICT",
            "dependency already exists",
        ),
        other => {
            tracing::error!(error=%other, "dependency graph storage error");
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
    fn resource_types_are_normalized() {
        assert_eq!(normalize_resource_type("service").unwrap(), "SERVICE");
        assert!(normalize_resource_type("CLIENT").is_err());
    }

    #[test]
    fn derived_edges_point_from_dependent_to_dependency() {
        let source = Uuid::new_v4();
        let target = Uuid::new_v4();
        let edge = derived_edge("SERVICE", source, "SERVER", target, "HOSTED_ON");
        assert_eq!(edge.source_id, source);
        assert_eq!(edge.target_id, target);
        assert_eq!(edge.origin, "DERIVED");
    }
}
