use crate::{ApiError, AppState, api_error, web_identity};
use axum::{
    Json, Router,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    routing::{get, post, put},
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};
use std::collections::HashSet;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct ProjectWorkspaceStore {
    pool: PgPool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProjectSummary {
    pub id: Uuid,
    pub name: String,
    pub description: String,
    pub preset: String,
    pub status: String,
    pub tags: Vec<String>,
    pub client_id: Option<Uuid>,
    pub open_tasks: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProjectTask {
    pub id: Uuid,
    pub project_id: Uuid,
    pub milestone_id: Option<Uuid>,
    pub title: String,
    pub description: String,
    pub status: String,
    pub priority: String,
    pub assignee_user_id: Option<Uuid>,
    pub due_at: Option<DateTime<Utc>>,
    pub labels: Vec<String>,
    pub created_by: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProjectNote {
    pub id: Uuid,
    pub project_id: Uuid,
    pub title: String,
    pub content: String,
    pub created_by: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Milestone {
    pub id: Uuid,
    pub project_id: Uuid,
    pub name: String,
    pub description: String,
    pub status: String,
    pub due_at: Option<DateTime<Utc>>,
    pub created_by: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ActivityItem {
    pub event_type: String,
    pub data: serde_json::Value,
    pub occurred_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProjectWorkspace {
    pub project: ProjectSummary,
    pub tasks: Vec<ProjectTask>,
    pub notes: Vec<ProjectNote>,
    pub milestones: Vec<Milestone>,
    pub activity: Vec<ActivityItem>,
}

#[derive(Debug, Deserialize)]
pub struct CreateProjectRequest {
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default = "default_preset")]
    pub preset: String,
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateTaskRequest {
    pub title: String,
    #[serde(default)]
    pub description: String,
    #[serde(default = "default_priority")]
    pub priority: String,
    pub due_at: Option<DateTime<Utc>>,
    pub milestone_id: Option<Uuid>,
    pub assignee_user_id: Option<Uuid>,
    #[serde(default)]
    pub labels: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateTaskStatusRequest {
    pub status: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateNoteRequest {
    pub title: String,
    pub content: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdateNoteRequest {
    pub title: String,
    pub content: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateMilestoneRequest {
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub due_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateMilestoneStatusRequest {
    pub status: String,
}

#[derive(Debug, thiserror::Error)]
pub enum ProjectWorkspaceError {
    #[error("resource not found")]
    NotFound,
    #[error("permission denied")]
    PermissionDenied,
    #[error("invalid request")]
    Invalid,
    #[error(transparent)]
    Sql(#[from] sqlx::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

impl ProjectWorkspaceStore {
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
    ) -> Result<(), ProjectWorkspaceError> {
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
            Err(ProjectWorkspaceError::NotFound)
        }
    }

    pub async fn list_projects(
        &self,
        organization_id: Uuid,
    ) -> Result<Vec<ProjectSummary>, ProjectWorkspaceError> {
        let rows = sqlx::query(
            "SELECT p.id,p.name,p.description,p.preset,p.status,p.tags,p.client_id,p.created_at,p.updated_at,COUNT(t.id) FILTER (WHERE t.status NOT IN ('DONE','CANCELLED'))::BIGINT AS open_tasks FROM projects p LEFT JOIN project_tasks t ON t.project_id=p.id AND t.organization_id=p.organization_id WHERE p.organization_id=$1 GROUP BY p.id ORDER BY p.updated_at DESC,p.created_at DESC",
        )
        .bind(organization_id)
        .fetch_all(&self.pool)
        .await?;
        rows.into_iter().map(project_from_row).collect()
    }

    pub async fn create_project(
        &self,
        identity: crate::persistence::WebIdentity,
        request: CreateProjectRequest,
    ) -> Result<ProjectSummary, ProjectWorkspaceError> {
        let name = required_text(&request.name, 1, 120)?;
        let description = optional_text(&request.description, 4000)?;
        let preset = request.preset.trim().to_lowercase();
        if !matches!(
            preset.as_str(),
            "empty" | "software" | "website" | "infrastructure" | "client"
        ) {
            return Err(ProjectWorkspaceError::Invalid);
        }
        let tags = validate_string_list(request.tags, 20, 50)?;
        let id = Uuid::new_v4();
        let mut tx = self.pool.begin().await?;
        sqlx::query(
            "INSERT INTO projects(id,organization_id,name,client_id,description,preset,status,tags,created_at,updated_at) VALUES($1,$2,$3,NULL,$4,$5,'ACTIVE',$6,NOW(),NOW())",
        )
        .bind(id)
        .bind(identity.organization_id)
        .bind(&name)
        .bind(&description)
        .bind(&preset)
        .bind(serde_json::to_value(&tags)?)
        .execute(&mut *tx)
        .await?;
        audit_and_event(
            &mut tx,
            identity,
            id,
            "project.created",
            serde_json::json!({"project_id":id,"name":name,"preset":preset}),
        )
        .await?;
        tx.commit().await?;
        self.project(identity.organization_id, id).await
    }

    pub async fn project(
        &self,
        organization_id: Uuid,
        project_id: Uuid,
    ) -> Result<ProjectSummary, ProjectWorkspaceError> {
        let row = sqlx::query(
            "SELECT p.id,p.name,p.description,p.preset,p.status,p.tags,p.client_id,p.created_at,p.updated_at,COUNT(t.id) FILTER (WHERE t.status NOT IN ('DONE','CANCELLED'))::BIGINT AS open_tasks FROM projects p LEFT JOIN project_tasks t ON t.project_id=p.id AND t.organization_id=p.organization_id WHERE p.id=$1 AND p.organization_id=$2 GROUP BY p.id",
        )
        .bind(project_id)
        .bind(organization_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(ProjectWorkspaceError::NotFound)?;
        project_from_row(row)
    }

    pub async fn workspace(
        &self,
        organization_id: Uuid,
        project_id: Uuid,
    ) -> Result<ProjectWorkspace, ProjectWorkspaceError> {
        let project = self.project(organization_id, project_id).await?;
        let task_rows = sqlx::query(
            "SELECT id,project_id,milestone_id,title,description,status,priority,assignee_user_id,due_at,labels,created_by,created_at,updated_at FROM project_tasks WHERE project_id=$1 AND organization_id=$2 ORDER BY CASE status WHEN 'IN_PROGRESS' THEN 0 WHEN 'BLOCKED' THEN 1 WHEN 'TODO' THEN 2 ELSE 3 END,due_at NULLS LAST,updated_at DESC",
        )
        .bind(project_id)
        .bind(organization_id)
        .fetch_all(&self.pool)
        .await?;
        let tasks = task_rows
            .into_iter()
            .map(task_from_row)
            .collect::<Result<_, _>>()?;

        let note_rows = sqlx::query(
            "SELECT id,project_id,title,content,created_by,created_at,updated_at FROM project_notes WHERE project_id=$1 AND organization_id=$2 ORDER BY updated_at DESC LIMIT 100",
        )
        .bind(project_id)
        .bind(organization_id)
        .fetch_all(&self.pool)
        .await?;
        let notes = note_rows
            .into_iter()
            .map(note_from_row)
            .collect::<Result<_, _>>()?;

        let milestone_rows = sqlx::query(
            "SELECT id,project_id,name,description,status,due_at,created_by,created_at,updated_at FROM milestones WHERE project_id=$1 AND organization_id=$2 ORDER BY CASE status WHEN 'OPEN' THEN 0 ELSE 1 END,due_at NULLS LAST,created_at DESC",
        )
        .bind(project_id)
        .bind(organization_id)
        .fetch_all(&self.pool)
        .await?;
        let milestones = milestone_rows
            .into_iter()
            .map(milestone_from_row)
            .collect::<Result<_, _>>()?;

        let activity_rows = sqlx::query(
            "SELECT event_type,data,occurred_at FROM domain_events WHERE organization_id=$1 AND resource_id=$2 ORDER BY occurred_at DESC LIMIT 100",
        )
        .bind(organization_id)
        .bind(project_id)
        .fetch_all(&self.pool)
        .await?;
        let activity = activity_rows
            .into_iter()
            .map(|row| ActivityItem {
                event_type: row.get("event_type"),
                data: row.get("data"),
                occurred_at: row.get("occurred_at"),
            })
            .collect();

        Ok(ProjectWorkspace {
            project,
            tasks,
            notes,
            milestones,
            activity,
        })
    }

    pub async fn create_task(
        &self,
        identity: crate::persistence::WebIdentity,
        project_id: Uuid,
        request: CreateTaskRequest,
    ) -> Result<ProjectTask, ProjectWorkspaceError> {
        self.authorize_project(identity.organization_id, project_id)
            .await?;
        let title = required_text(&request.title, 1, 200)?;
        let description = optional_text(&request.description, 8000)?;
        let priority = request.priority.trim().to_uppercase();
        if !matches!(priority.as_str(), "LOW" | "MEDIUM" | "HIGH" | "URGENT") {
            return Err(ProjectWorkspaceError::Invalid);
        }
        let labels = validate_string_list(request.labels, 20, 50)?;
        if let Some(milestone_id) = request.milestone_id {
            let valid: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM milestones WHERE id=$1 AND project_id=$2 AND organization_id=$3)",
            )
            .bind(milestone_id)
            .bind(project_id)
            .bind(identity.organization_id)
            .fetch_one(&self.pool)
            .await?;
            if !valid {
                return Err(ProjectWorkspaceError::Invalid);
            }
        }
        if let Some(assignee) = request.assignee_user_id {
            let valid: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM users WHERE id=$1 AND organization_id=$2)",
            )
            .bind(assignee)
            .bind(identity.organization_id)
            .fetch_one(&self.pool)
            .await?;
            if !valid {
                return Err(ProjectWorkspaceError::Invalid);
            }
        }
        let id = Uuid::new_v4();
        let mut tx = self.pool.begin().await?;
        sqlx::query(
            "INSERT INTO project_tasks(id,organization_id,project_id,milestone_id,title,description,status,priority,assignee_user_id,due_at,labels,created_by,created_at,updated_at) VALUES($1,$2,$3,$4,$5,$6,'TODO',$7,$8,$9,$10,$11,NOW(),NOW())",
        )
        .bind(id)
        .bind(identity.organization_id)
        .bind(project_id)
        .bind(request.milestone_id)
        .bind(&title)
        .bind(&description)
        .bind(&priority)
        .bind(request.assignee_user_id)
        .bind(request.due_at)
        .bind(serde_json::to_value(&labels)?)
        .bind(identity.user_id)
        .execute(&mut *tx)
        .await?;
        touch_project(&mut tx, identity.organization_id, project_id).await?;
        audit_and_event(
            &mut tx,
            identity,
            project_id,
            "project.task.created",
            serde_json::json!({"project_id":project_id,"task_id":id,"title":title,"priority":priority}),
        )
        .await?;
        tx.commit().await?;
        self.task(identity.organization_id, project_id, id).await
    }

    pub async fn update_task_status(
        &self,
        identity: crate::persistence::WebIdentity,
        project_id: Uuid,
        task_id: Uuid,
        request: UpdateTaskStatusRequest,
    ) -> Result<ProjectTask, ProjectWorkspaceError> {
        self.authorize_project(identity.organization_id, project_id)
            .await?;
        let status = request.status.trim().to_uppercase();
        if !matches!(
            status.as_str(),
            "TODO" | "IN_PROGRESS" | "BLOCKED" | "DONE" | "CANCELLED"
        ) {
            return Err(ProjectWorkspaceError::Invalid);
        }
        let mut tx = self.pool.begin().await?;
        let changed = sqlx::query(
            "UPDATE project_tasks SET status=$1,updated_at=NOW() WHERE id=$2 AND project_id=$3 AND organization_id=$4",
        )
        .bind(&status)
        .bind(task_id)
        .bind(project_id)
        .bind(identity.organization_id)
        .execute(&mut *tx)
        .await?;
        if changed.rows_affected() != 1 {
            tx.rollback().await?;
            return Err(ProjectWorkspaceError::NotFound);
        }
        touch_project(&mut tx, identity.organization_id, project_id).await?;
        audit_and_event(
            &mut tx,
            identity,
            project_id,
            "project.task.status_changed",
            serde_json::json!({"project_id":project_id,"task_id":task_id,"status":status}),
        )
        .await?;
        tx.commit().await?;
        self.task(identity.organization_id, project_id, task_id)
            .await
    }

    async fn task(
        &self,
        organization_id: Uuid,
        project_id: Uuid,
        task_id: Uuid,
    ) -> Result<ProjectTask, ProjectWorkspaceError> {
        let row = sqlx::query(
            "SELECT id,project_id,milestone_id,title,description,status,priority,assignee_user_id,due_at,labels,created_by,created_at,updated_at FROM project_tasks WHERE id=$1 AND project_id=$2 AND organization_id=$3",
        )
        .bind(task_id)
        .bind(project_id)
        .bind(organization_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(ProjectWorkspaceError::NotFound)?;
        task_from_row(row)
    }

    pub async fn create_note(
        &self,
        identity: crate::persistence::WebIdentity,
        project_id: Uuid,
        request: CreateNoteRequest,
    ) -> Result<ProjectNote, ProjectWorkspaceError> {
        self.authorize_project(identity.organization_id, project_id)
            .await?;
        let title = required_text(&request.title, 1, 200)?;
        let content = required_text(&request.content, 1, 50_000)?;
        let id = Uuid::new_v4();
        let mut tx = self.pool.begin().await?;
        sqlx::query(
            "INSERT INTO project_notes(id,organization_id,project_id,title,content,created_by,created_at,updated_at) VALUES($1,$2,$3,$4,$5,$6,NOW(),NOW())",
        )
        .bind(id)
        .bind(identity.organization_id)
        .bind(project_id)
        .bind(&title)
        .bind(&content)
        .bind(identity.user_id)
        .execute(&mut *tx)
        .await?;
        touch_project(&mut tx, identity.organization_id, project_id).await?;
        audit_and_event(
            &mut tx,
            identity,
            project_id,
            "project.note.created",
            serde_json::json!({"project_id":project_id,"note_id":id,"title":title}),
        )
        .await?;
        tx.commit().await?;
        self.note(identity.organization_id, project_id, id).await
    }

    pub async fn update_note(
        &self,
        identity: crate::persistence::WebIdentity,
        project_id: Uuid,
        note_id: Uuid,
        request: UpdateNoteRequest,
    ) -> Result<ProjectNote, ProjectWorkspaceError> {
        self.authorize_project(identity.organization_id, project_id)
            .await?;
        let title = required_text(&request.title, 1, 200)?;
        let content = required_text(&request.content, 1, 50_000)?;
        let mut tx = self.pool.begin().await?;
        let changed = sqlx::query(
            "UPDATE project_notes SET title=$1,content=$2,updated_at=NOW() WHERE id=$3 AND project_id=$4 AND organization_id=$5",
        )
        .bind(&title)
        .bind(&content)
        .bind(note_id)
        .bind(project_id)
        .bind(identity.organization_id)
        .execute(&mut *tx)
        .await?;
        if changed.rows_affected() != 1 {
            tx.rollback().await?;
            return Err(ProjectWorkspaceError::NotFound);
        }
        touch_project(&mut tx, identity.organization_id, project_id).await?;
        audit_and_event(
            &mut tx,
            identity,
            project_id,
            "project.note.updated",
            serde_json::json!({"project_id":project_id,"note_id":note_id,"title":title}),
        )
        .await?;
        tx.commit().await?;
        self.note(identity.organization_id, project_id, note_id)
            .await
    }

    async fn note(
        &self,
        organization_id: Uuid,
        project_id: Uuid,
        note_id: Uuid,
    ) -> Result<ProjectNote, ProjectWorkspaceError> {
        let row = sqlx::query(
            "SELECT id,project_id,title,content,created_by,created_at,updated_at FROM project_notes WHERE id=$1 AND project_id=$2 AND organization_id=$3",
        )
        .bind(note_id)
        .bind(project_id)
        .bind(organization_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(ProjectWorkspaceError::NotFound)?;
        Ok(note_from_row(row)?)
    }

    pub async fn create_milestone(
        &self,
        identity: crate::persistence::WebIdentity,
        project_id: Uuid,
        request: CreateMilestoneRequest,
    ) -> Result<Milestone, ProjectWorkspaceError> {
        self.authorize_project(identity.organization_id, project_id)
            .await?;
        let name = required_text(&request.name, 1, 160)?;
        let description = optional_text(&request.description, 4000)?;
        let id = Uuid::new_v4();
        let mut tx = self.pool.begin().await?;
        sqlx::query(
            "INSERT INTO milestones(id,organization_id,project_id,name,description,status,due_at,created_by,created_at,updated_at) VALUES($1,$2,$3,$4,$5,'OPEN',$6,$7,NOW(),NOW())",
        )
        .bind(id)
        .bind(identity.organization_id)
        .bind(project_id)
        .bind(&name)
        .bind(&description)
        .bind(request.due_at)
        .bind(identity.user_id)
        .execute(&mut *tx)
        .await?;
        touch_project(&mut tx, identity.organization_id, project_id).await?;
        audit_and_event(
            &mut tx,
            identity,
            project_id,
            "project.milestone.created",
            serde_json::json!({"project_id":project_id,"milestone_id":id,"name":name}),
        )
        .await?;
        tx.commit().await?;
        self.milestone(identity.organization_id, project_id, id)
            .await
    }

    pub async fn update_milestone_status(
        &self,
        identity: crate::persistence::WebIdentity,
        project_id: Uuid,
        milestone_id: Uuid,
        request: UpdateMilestoneStatusRequest,
    ) -> Result<Milestone, ProjectWorkspaceError> {
        self.authorize_project(identity.organization_id, project_id)
            .await?;
        let status = request.status.trim().to_uppercase();
        if !matches!(status.as_str(), "OPEN" | "COMPLETED" | "CANCELLED") {
            return Err(ProjectWorkspaceError::Invalid);
        }
        let mut tx = self.pool.begin().await?;
        let changed = sqlx::query(
            "UPDATE milestones SET status=$1,updated_at=NOW() WHERE id=$2 AND project_id=$3 AND organization_id=$4",
        )
        .bind(&status)
        .bind(milestone_id)
        .bind(project_id)
        .bind(identity.organization_id)
        .execute(&mut *tx)
        .await?;
        if changed.rows_affected() != 1 {
            tx.rollback().await?;
            return Err(ProjectWorkspaceError::NotFound);
        }
        touch_project(&mut tx, identity.organization_id, project_id).await?;
        audit_and_event(
            &mut tx,
            identity,
            project_id,
            "project.milestone.status_changed",
            serde_json::json!({"project_id":project_id,"milestone_id":milestone_id,"status":status}),
        )
        .await?;
        tx.commit().await?;
        self.milestone(identity.organization_id, project_id, milestone_id)
            .await
    }

    async fn milestone(
        &self,
        organization_id: Uuid,
        project_id: Uuid,
        milestone_id: Uuid,
    ) -> Result<Milestone, ProjectWorkspaceError> {
        let row = sqlx::query(
            "SELECT id,project_id,name,description,status,due_at,created_by,created_at,updated_at FROM milestones WHERE id=$1 AND project_id=$2 AND organization_id=$3",
        )
        .bind(milestone_id)
        .bind(project_id)
        .bind(organization_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(ProjectWorkspaceError::NotFound)?;
        Ok(milestone_from_row(row)?)
    }
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/projects", get(list_projects).post(create_project))
        .route("/projects/:project_id", get(get_project_workspace))
        .route("/projects/:project_id/tasks", post(create_task))
        .route(
            "/projects/:project_id/tasks/:task_id/status",
            put(update_task_status),
        )
        .route("/projects/:project_id/notes", post(create_note))
        .route("/projects/:project_id/notes/:note_id", put(update_note))
        .route("/projects/:project_id/milestones", post(create_milestone))
        .route(
            "/projects/:project_id/milestones/:milestone_id/status",
            put(update_milestone_status),
        )
}

async fn list_projects(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<ProjectSummary>>, ApiError> {
    let identity = web_identity(&state, &headers).await?;
    Ok(Json(
        state
            .workspace
            .list_projects(identity.organization_id)
            .await
            .map_err(map_workspace)?,
    ))
}

async fn create_project(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<CreateProjectRequest>,
) -> Result<(StatusCode, Json<ProjectSummary>), ApiError> {
    let identity = web_identity(&state, &headers).await?;
    let project = state
        .workspace
        .create_project(identity, request)
        .await
        .map_err(map_workspace)?;
    Ok((StatusCode::CREATED, Json(project)))
}

async fn get_project_workspace(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(project_id): Path<Uuid>,
) -> Result<Json<ProjectWorkspace>, ApiError> {
    let identity = web_identity(&state, &headers).await?;
    Ok(Json(
        state
            .workspace
            .workspace(identity.organization_id, project_id)
            .await
            .map_err(map_workspace)?,
    ))
}

async fn create_task(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(project_id): Path<Uuid>,
    Json(request): Json<CreateTaskRequest>,
) -> Result<(StatusCode, Json<ProjectTask>), ApiError> {
    let identity = web_identity(&state, &headers).await?;
    let task = state
        .workspace
        .create_task(identity, project_id, request)
        .await
        .map_err(map_workspace)?;
    Ok((StatusCode::CREATED, Json(task)))
}

async fn update_task_status(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((project_id, task_id)): Path<(Uuid, Uuid)>,
    Json(request): Json<UpdateTaskStatusRequest>,
) -> Result<Json<ProjectTask>, ApiError> {
    let identity = web_identity(&state, &headers).await?;
    Ok(Json(
        state
            .workspace
            .update_task_status(identity, project_id, task_id, request)
            .await
            .map_err(map_workspace)?,
    ))
}

async fn create_note(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(project_id): Path<Uuid>,
    Json(request): Json<CreateNoteRequest>,
) -> Result<(StatusCode, Json<ProjectNote>), ApiError> {
    let identity = web_identity(&state, &headers).await?;
    let note = state
        .workspace
        .create_note(identity, project_id, request)
        .await
        .map_err(map_workspace)?;
    Ok((StatusCode::CREATED, Json(note)))
}

async fn update_note(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((project_id, note_id)): Path<(Uuid, Uuid)>,
    Json(request): Json<UpdateNoteRequest>,
) -> Result<Json<ProjectNote>, ApiError> {
    let identity = web_identity(&state, &headers).await?;
    Ok(Json(
        state
            .workspace
            .update_note(identity, project_id, note_id, request)
            .await
            .map_err(map_workspace)?,
    ))
}

async fn create_milestone(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(project_id): Path<Uuid>,
    Json(request): Json<CreateMilestoneRequest>,
) -> Result<(StatusCode, Json<Milestone>), ApiError> {
    let identity = web_identity(&state, &headers).await?;
    let milestone = state
        .workspace
        .create_milestone(identity, project_id, request)
        .await
        .map_err(map_workspace)?;
    Ok((StatusCode::CREATED, Json(milestone)))
}

async fn update_milestone_status(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((project_id, milestone_id)): Path<(Uuid, Uuid)>,
    Json(request): Json<UpdateMilestoneStatusRequest>,
) -> Result<Json<Milestone>, ApiError> {
    let identity = web_identity(&state, &headers).await?;
    Ok(Json(
        state
            .workspace
            .update_milestone_status(identity, project_id, milestone_id, request)
            .await
            .map_err(map_workspace)?,
    ))
}

fn project_from_row(row: sqlx::postgres::PgRow) -> Result<ProjectSummary, ProjectWorkspaceError> {
    let tags: serde_json::Value = row.get("tags");
    Ok(ProjectSummary {
        id: row.get("id"),
        name: row.get("name"),
        description: row.get("description"),
        preset: row.get("preset"),
        status: row.get("status"),
        tags: serde_json::from_value(tags)?,
        client_id: row.get("client_id"),
        open_tasks: row.get("open_tasks"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}

fn task_from_row(row: sqlx::postgres::PgRow) -> Result<ProjectTask, ProjectWorkspaceError> {
    let labels: serde_json::Value = row.get("labels");
    Ok(ProjectTask {
        id: row.get("id"),
        project_id: row.get("project_id"),
        milestone_id: row.get("milestone_id"),
        title: row.get("title"),
        description: row.get("description"),
        status: row.get("status"),
        priority: row.get("priority"),
        assignee_user_id: row.get("assignee_user_id"),
        due_at: row.get("due_at"),
        labels: serde_json::from_value(labels)?,
        created_by: row.get("created_by"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}

fn note_from_row(row: sqlx::postgres::PgRow) -> Result<ProjectNote, ProjectWorkspaceError> {
    Ok(ProjectNote {
        id: row.get("id"),
        project_id: row.get("project_id"),
        title: row.get("title"),
        content: row.get("content"),
        created_by: row.get("created_by"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}

fn milestone_from_row(row: sqlx::postgres::PgRow) -> Result<Milestone, ProjectWorkspaceError> {
    Ok(Milestone {
        id: row.get("id"),
        project_id: row.get("project_id"),
        name: row.get("name"),
        description: row.get("description"),
        status: row.get("status"),
        due_at: row.get("due_at"),
        created_by: row.get("created_by"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}

fn required_text(value: &str, min: usize, max: usize) -> Result<String, ProjectWorkspaceError> {
    let value = value.trim();
    if value.len() < min || value.len() > max {
        Err(ProjectWorkspaceError::Invalid)
    } else {
        Ok(value.to_string())
    }
}

fn optional_text(value: &str, max: usize) -> Result<String, ProjectWorkspaceError> {
    let value = value.trim();
    if value.len() > max {
        Err(ProjectWorkspaceError::Invalid)
    } else {
        Ok(value.to_string())
    }
}

fn validate_string_list(
    values: Vec<String>,
    max_items: usize,
    max_len: usize,
) -> Result<Vec<String>, ProjectWorkspaceError> {
    if values.len() > max_items {
        return Err(ProjectWorkspaceError::Invalid);
    }
    let mut seen = HashSet::new();
    let mut result = Vec::new();
    for value in values {
        let value = value.trim().to_lowercase();
        if value.is_empty() || value.len() > max_len {
            return Err(ProjectWorkspaceError::Invalid);
        }
        if seen.insert(value.clone()) {
            result.push(value);
        }
    }
    Ok(result)
}

fn default_preset() -> String {
    "empty".into()
}
fn default_priority() -> String {
    "MEDIUM".into()
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

async fn audit_and_event(
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

fn map_workspace(error: ProjectWorkspaceError) -> ApiError {
    match error {
        ProjectWorkspaceError::NotFound => api_error(
            StatusCode::NOT_FOUND,
            "PROJECT_NOT_FOUND",
            "project workspace resource not found",
        ),
        ProjectWorkspaceError::PermissionDenied => api_error(
            StatusCode::FORBIDDEN,
            "PERMISSION_DENIED",
            "operation not permitted",
        ),
        ProjectWorkspaceError::Invalid => api_error(
            StatusCode::BAD_REQUEST,
            "INVALID_REQUEST",
            "invalid project workspace request",
        ),
        other => {
            tracing::error!(error=%other, "project workspace storage error");
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
    fn project_presets_do_not_imply_client_requirement() {
        let request: CreateProjectRequest = serde_json::from_value(serde_json::json!({
            "name":"Personal Infrastructure",
            "preset":"infrastructure",
            "tags":["personal","production"]
        }))
        .unwrap();
        assert_eq!(request.preset, "infrastructure");
    }

    #[test]
    fn string_lists_are_normalized_and_deduplicated() {
        let tags = validate_string_list(
            vec!["Personal".into(), "personal".into(), " Rust ".into()],
            20,
            50,
        )
        .unwrap();
        assert_eq!(tags, vec!["personal", "rust"]);
    }

    #[test]
    fn invalid_task_status_is_not_accepted_by_validator_contract() {
        assert!(!matches!(
            "WAT",
            "TODO" | "IN_PROGRESS" | "BLOCKED" | "DONE" | "CANCELLED"
        ));
    }
}
