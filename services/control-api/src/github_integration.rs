use crate::{ApiError, AppState, api_error, web_identity};
use axum::{
    Json, Router,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    routing::{delete, get, post},
};
use chrono::{DateTime, Utc};
use reqwest::{Client, Url};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use sqlx::{PgPool, Row};
use std::{sync::Arc, time::Duration};
use uuid::Uuid;

const GITHUB_API_VERSION: &str = "2026-03-10";
const MAX_LIST_ITEMS: usize = 100;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CommitSummary {
    pub sha: String,
    pub message: String,
    pub committed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CiSummary {
    pub state: String,
    pub total_checks: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RepositorySnapshot {
    pub default_branch: String,
    pub latest_commit: Option<CommitSummary>,
    pub open_pull_requests: u32,
    pub open_issues: u32,
    pub counts_truncated: bool,
    pub ci: CiSummary,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RepositoryLink {
    pub id: Uuid,
    pub project_id: Uuid,
    pub provider: String,
    pub owner: String,
    pub name: String,
    pub html_url: String,
    pub default_branch: String,
    pub visibility: String,
    pub snapshot: RepositorySnapshot,
    pub sync_status: String,
    pub sync_error: Option<String>,
    pub last_synced_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct LinkRepositoryRequest {
    pub owner: String,
    pub name: String,
}

#[derive(Debug, Clone)]
struct SyncedRepository {
    html_url: String,
    default_branch: String,
    visibility: String,
    snapshot: RepositorySnapshot,
}

pub trait RepositoryProvider {
    async fn fetch_repository(
        &self,
        owner: &str,
        name: &str,
    ) -> Result<SyncedRepository, GitHubError>;
}

#[derive(Debug, Clone)]
pub struct GitHubProvider {
    client: Client,
    token: Option<Arc<str>>,
}

#[derive(Debug, thiserror::Error)]
pub enum GitHubError {
    #[error("repository not found")]
    NotFound,
    #[error("GitHub permission denied")]
    PermissionDenied,
    #[error("GitHub API unavailable: {0}")]
    Unavailable(String),
    #[error("invalid GitHub response: {0}")]
    InvalidResponse(String),
}

impl GitHubProvider {
    pub fn from_env() -> Result<Self, reqwest::Error> {
        let client = Client::builder()
            .user_agent("Argus/0.1")
            .timeout(Duration::from_secs(15))
            .build()?;
        let token = std::env::var("ARGUS_GITHUB_TOKEN")
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .map(Arc::<str>::from);
        Ok(Self { client, token })
    }

    fn url(&self, segments: &[&str], query: &[(&str, &str)]) -> Result<Url, GitHubError> {
        let mut url = Url::parse("https://api.github.com/")
            .map_err(|error| GitHubError::InvalidResponse(error.to_string()))?;
        url.path_segments_mut()
            .map_err(|_| GitHubError::InvalidResponse("invalid GitHub API base URL".into()))?
            .extend(segments.iter().copied());
        if !query.is_empty() {
            url.query_pairs_mut().extend_pairs(query.iter().copied());
        }
        Ok(url)
    }

    async fn get<T: DeserializeOwned>(
        &self,
        segments: &[&str],
        query: &[(&str, &str)],
    ) -> Result<T, GitHubError> {
        let url = self.url(segments, query)?;
        let mut request = self
            .client
            .get(url)
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", GITHUB_API_VERSION);
        if let Some(token) = &self.token {
            request = request.bearer_auth(token.as_ref());
        }
        let response = request
            .send()
            .await
            .map_err(|error| GitHubError::Unavailable(error.to_string()))?;
        match response.status() {
            StatusCode::NOT_FOUND => return Err(GitHubError::NotFound),
            StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => {
                return Err(GitHubError::PermissionDenied);
            }
            status if !status.is_success() => {
                return Err(GitHubError::Unavailable(format!(
                    "HTTP {}",
                    status.as_u16()
                )));
            }
            _ => {}
        }
        response
            .json::<T>()
            .await
            .map_err(|error| GitHubError::InvalidResponse(error.to_string()))
    }
}

#[derive(Debug, Deserialize)]
struct GitHubRepository {
    html_url: String,
    default_branch: String,
    #[serde(default)]
    visibility: String,
    #[serde(default)]
    private: bool,
}

#[derive(Debug, Deserialize)]
struct GitHubBranch {
    commit: GitHubBranchCommit,
}
#[derive(Debug, Deserialize)]
struct GitHubBranchCommit {
    sha: String,
    commit: GitHubCommitDetails,
}
#[derive(Debug, Deserialize)]
struct GitHubCommitDetails {
    message: String,
    author: Option<GitHubCommitAuthor>,
    committer: Option<GitHubCommitAuthor>,
}
#[derive(Debug, Deserialize)]
struct GitHubCommitAuthor {
    date: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
struct GitHubCheckRuns {
    total_count: u32,
    #[serde(default)]
    check_runs: Vec<GitHubCheckRun>,
}
#[derive(Debug, Deserialize)]
struct GitHubCheckRun {
    status: String,
    conclusion: Option<String>,
}

impl RepositoryProvider for GitHubProvider {
    async fn fetch_repository(
        &self,
        owner: &str,
        name: &str,
    ) -> Result<SyncedRepository, GitHubError> {
        let repository: GitHubRepository = self
            .get(&["repos", owner, name], &[])
            .await?;
        let branch: GitHubBranch = self
            .get(
                &["repos", owner, name, "branches", &repository.default_branch],
                &[],
            )
            .await?;

        let committed_at = branch
            .commit
            .commit
            .committer
            .as_ref()
            .or(branch.commit.commit.author.as_ref())
            .map(|author| author.date);
        let latest_commit = Some(CommitSummary {
            sha: branch.commit.sha.clone(),
            message: branch.commit.commit.message,
            committed_at,
        });

        let mut warnings = Vec::new();
        let pulls: Vec<serde_json::Value> = match self
            .get(
                &["repos", owner, name, "pulls"],
                &[("state", "open"), ("per_page", "100")],
            )
            .await
        {
            Ok(value) => value,
            Err(error) => {
                warnings.push(format!("pull requests unavailable: {error}"));
                Vec::new()
            }
        };
        let issues: Vec<serde_json::Value> = match self
            .get(
                &["repos", owner, name, "issues"],
                &[("state", "open"), ("per_page", "100")],
            )
            .await
        {
            Ok(value) => value,
            Err(error) => {
                warnings.push(format!("issues unavailable: {error}"));
                Vec::new()
            }
        };
        let issue_count = issues
            .iter()
            .filter(|item| item.get("pull_request").is_none())
            .count();
        let counts_truncated = pulls.len() >= MAX_LIST_ITEMS || issues.len() >= MAX_LIST_ITEMS;

        let ci = match self
            .get::<GitHubCheckRuns>(
                &["repos", owner, name, "commits", &branch.commit.sha, "check-runs"],
                &[("per_page", "100")],
            )
            .await
        {
            Ok(checks) => summarize_checks(&checks),
            Err(error) => {
                warnings.push(format!("checks unavailable: {error}"));
                CiSummary {
                    state: "UNAVAILABLE".into(),
                    total_checks: 0,
                }
            }
        };

        let visibility = if repository.visibility.is_empty() {
            if repository.private {
                "private".to_string()
            } else {
                "public".to_string()
            }
        } else {
            repository.visibility
        };

        Ok(SyncedRepository {
            html_url: repository.html_url,
            default_branch: repository.default_branch.clone(),
            visibility,
            snapshot: RepositorySnapshot {
                default_branch: repository.default_branch,
                latest_commit,
                open_pull_requests: pulls.len().try_into().unwrap_or(u32::MAX),
                open_issues: issue_count.try_into().unwrap_or(u32::MAX),
                counts_truncated,
                ci,
                warnings,
            },
        })
    }
}

fn summarize_checks(checks: &GitHubCheckRuns) -> CiSummary {
    if checks.total_count == 0 || checks.check_runs.is_empty() {
        return CiSummary {
            state: "NONE".into(),
            total_checks: checks.total_count,
        };
    }
    if checks.check_runs.iter().any(|check| check.status != "completed") {
        return CiSummary {
            state: "RUNNING".into(),
            total_checks: checks.total_count,
        };
    }
    let failed = checks.check_runs.iter().any(|check| {
        matches!(
            check.conclusion.as_deref(),
            Some("failure" | "timed_out" | "cancelled" | "action_required" | "stale")
        )
    });
    CiSummary {
        state: if failed { "FAILURE" } else { "SUCCESS" }.into(),
        total_checks: checks.total_count,
    }
}

#[derive(Debug, Clone)]
pub struct GitHubIntegrationStore {
    pool: PgPool,
    provider: GitHubProvider,
}

#[derive(Debug, thiserror::Error)]
pub enum IntegrationError {
    #[error("repository link not found")]
    NotFound,
    #[error("invalid repository reference")]
    Invalid,
    #[error("repository already linked")]
    Conflict,
    #[error(transparent)]
    Provider(#[from] GitHubError),
    #[error(transparent)]
    Sql(#[from] sqlx::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

impl GitHubIntegrationStore {
    pub async fn connect(
        database_url: &str,
        provider: GitHubProvider,
    ) -> Result<Self, sqlx::Error> {
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(10)
            .connect(database_url)
            .await?;
        Ok(Self { pool, provider })
    }

    async fn authorize_project(
        &self,
        organization_id: Uuid,
        project_id: Uuid,
    ) -> Result<(), IntegrationError> {
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
            Err(IntegrationError::NotFound)
        }
    }

    pub async fn list(
        &self,
        organization_id: Uuid,
        project_id: Uuid,
    ) -> Result<Vec<RepositoryLink>, IntegrationError> {
        self.authorize_project(organization_id, project_id).await?;
        let rows = sqlx::query(
            "SELECT id,project_id,provider,owner,name,html_url,default_branch,visibility,snapshot,sync_status,sync_error,last_synced_at,created_at,updated_at FROM project_repositories WHERE organization_id=$1 AND project_id=$2 ORDER BY updated_at DESC",
        )
        .bind(organization_id)
        .bind(project_id)
        .fetch_all(&self.pool)
        .await?;
        rows.into_iter().map(repository_from_row).collect()
    }

    pub async fn link(
        &self,
        identity: crate::persistence::WebIdentity,
        project_id: Uuid,
        request: LinkRepositoryRequest,
    ) -> Result<RepositoryLink, IntegrationError> {
        self.authorize_project(identity.organization_id, project_id).await?;
        let owner = validate_segment(&request.owner, 100)?;
        let name = validate_segment(&request.name, 100)?;
        let synced = self.provider.fetch_repository(&owner, &name).await?;
        let id = Uuid::new_v4();
        let snapshot = serde_json::to_value(&synced.snapshot)?;
        let mut tx = self.pool.begin().await?;
        let inserted = sqlx::query(
            "INSERT INTO project_repositories(id,organization_id,project_id,provider,owner,name,html_url,default_branch,visibility,snapshot,sync_status,sync_error,last_synced_at,created_by,created_at,updated_at) VALUES($1,$2,$3,'github',$4,$5,$6,$7,$8,$9,'SYNCED',NULL,NOW(),$10,NOW(),NOW()) ON CONFLICT(project_id,provider,owner,name) DO NOTHING",
        )
        .bind(id)
        .bind(identity.organization_id)
        .bind(project_id)
        .bind(&owner)
        .bind(&name)
        .bind(&synced.html_url)
        .bind(&synced.default_branch)
        .bind(&synced.visibility)
        .bind(snapshot)
        .bind(identity.user_id)
        .execute(&mut *tx)
        .await?;
        if inserted.rows_affected() != 1 {
            tx.rollback().await?;
            return Err(IntegrationError::Conflict);
        }
        audit_event(
            &mut tx,
            identity,
            project_id,
            "repository.linked",
            serde_json::json!({"repository_id":id,"provider":"github","owner":owner,"name":name}),
        )
        .await?;
        touch_project(&mut tx, identity.organization_id, project_id).await?;
        tx.commit().await?;
        self.get(identity.organization_id, project_id, id).await
    }

    pub async fn sync(
        &self,
        identity: crate::persistence::WebIdentity,
        project_id: Uuid,
        repository_id: Uuid,
    ) -> Result<RepositoryLink, IntegrationError> {
        let current = self
            .get(identity.organization_id, project_id, repository_id)
            .await?;
        match self
            .provider
            .fetch_repository(&current.owner, &current.name)
            .await
        {
            Ok(synced) => {
                let mut tx = self.pool.begin().await?;
                sqlx::query(
                    "UPDATE project_repositories SET html_url=$1,default_branch=$2,visibility=$3,snapshot=$4,sync_status='SYNCED',sync_error=NULL,last_synced_at=NOW(),updated_at=NOW() WHERE id=$5 AND project_id=$6 AND organization_id=$7",
                )
                .bind(&synced.html_url)
                .bind(&synced.default_branch)
                .bind(&synced.visibility)
                .bind(serde_json::to_value(&synced.snapshot)?)
                .bind(repository_id)
                .bind(project_id)
                .bind(identity.organization_id)
                .execute(&mut *tx)
                .await?;
                audit_event(
                    &mut tx,
                    identity,
                    project_id,
                    "repository.synced",
                    serde_json::json!({"repository_id":repository_id}),
                )
                .await?;
                tx.commit().await?;
            }
            Err(error) => {
                let message = truncate_error(&error.to_string());
                let mut tx = self.pool.begin().await?;
                sqlx::query(
                    "UPDATE project_repositories SET sync_status='ERROR',sync_error=$1,updated_at=NOW() WHERE id=$2 AND project_id=$3 AND organization_id=$4",
                )
                .bind(&message)
                .bind(repository_id)
                .bind(project_id)
                .bind(identity.organization_id)
                .execute(&mut *tx)
                .await?;
                audit_event(
                    &mut tx,
                    identity,
                    project_id,
                    "repository.sync_failed",
                    serde_json::json!({"repository_id":repository_id,"error":message}),
                )
                .await?;
                tx.commit().await?;
            }
        }
        self.get(identity.organization_id, project_id, repository_id)
            .await
    }

    pub async fn unlink(
        &self,
        identity: crate::persistence::WebIdentity,
        project_id: Uuid,
        repository_id: Uuid,
    ) -> Result<(), IntegrationError> {
        self.authorize_project(identity.organization_id, project_id).await?;
        let mut tx = self.pool.begin().await?;
        let deleted = sqlx::query(
            "DELETE FROM project_repositories WHERE id=$1 AND project_id=$2 AND organization_id=$3",
        )
        .bind(repository_id)
        .bind(project_id)
        .bind(identity.organization_id)
        .execute(&mut *tx)
        .await?;
        if deleted.rows_affected() != 1 {
            tx.rollback().await?;
            return Err(IntegrationError::NotFound);
        }
        audit_event(
            &mut tx,
            identity,
            project_id,
            "repository.unlinked",
            serde_json::json!({"repository_id":repository_id}),
        )
        .await?;
        touch_project(&mut tx, identity.organization_id, project_id).await?;
        tx.commit().await?;
        Ok(())
    }

    async fn get(
        &self,
        organization_id: Uuid,
        project_id: Uuid,
        repository_id: Uuid,
    ) -> Result<RepositoryLink, IntegrationError> {
        let row = sqlx::query(
            "SELECT id,project_id,provider,owner,name,html_url,default_branch,visibility,snapshot,sync_status,sync_error,last_synced_at,created_at,updated_at FROM project_repositories WHERE id=$1 AND organization_id=$2 AND project_id=$3",
        )
        .bind(repository_id)
        .bind(organization_id)
        .bind(project_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(IntegrationError::NotFound)?;
        repository_from_row(row)
    }
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/projects/:project_id/repositories",
            get(list_repositories).post(link_repository),
        )
        .route(
            "/projects/:project_id/repositories/:repository_id",
            delete(unlink_repository),
        )
        .route(
            "/projects/:project_id/repositories/:repository_id/sync",
            post(sync_repository),
        )
}

async fn list_repositories(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(project_id): Path<Uuid>,
) -> Result<Json<Vec<RepositoryLink>>, ApiError> {
    let identity = web_identity(&state, &headers).await?;
    Ok(Json(
        state
            .github
            .list(identity.organization_id, project_id)
            .await
            .map_err(map_integration)?,
    ))
}

async fn link_repository(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(project_id): Path<Uuid>,
    Json(request): Json<LinkRepositoryRequest>,
) -> Result<(StatusCode, Json<RepositoryLink>), ApiError> {
    let identity = web_identity(&state, &headers).await?;
    let repository = state
        .github
        .link(identity, project_id, request)
        .await
        .map_err(map_integration)?;
    Ok((StatusCode::CREATED, Json(repository)))
}

async fn sync_repository(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((project_id, repository_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<RepositoryLink>, ApiError> {
    let identity = web_identity(&state, &headers).await?;
    Ok(Json(
        state
            .github
            .sync(identity, project_id, repository_id)
            .await
            .map_err(map_integration)?,
    ))
}

async fn unlink_repository(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((project_id, repository_id)): Path<(Uuid, Uuid)>,
) -> Result<StatusCode, ApiError> {
    let identity = web_identity(&state, &headers).await?;
    state
        .github
        .unlink(identity, project_id, repository_id)
        .await
        .map_err(map_integration)?;
    Ok(StatusCode::NO_CONTENT)
}

fn repository_from_row(row: sqlx::postgres::PgRow) -> Result<RepositoryLink, IntegrationError> {
    let snapshot: serde_json::Value = row.get("snapshot");
    Ok(RepositoryLink {
        id: row.get("id"),
        project_id: row.get("project_id"),
        provider: row.get("provider"),
        owner: row.get("owner"),
        name: row.get("name"),
        html_url: row.get("html_url"),
        default_branch: row.get("default_branch"),
        visibility: row.get("visibility"),
        snapshot: serde_json::from_value(snapshot)?,
        sync_status: row.get("sync_status"),
        sync_error: row.get("sync_error"),
        last_synced_at: row.get("last_synced_at"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}

fn validate_segment(value: &str, max: usize) -> Result<String, IntegrationError> {
    let value = value.trim();
    let valid = !value.is_empty()
        && value.len() <= max
        && value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'));
    if valid {
        Ok(value.to_string())
    } else {
        Err(IntegrationError::Invalid)
    }
}

fn truncate_error(value: &str) -> String {
    value.chars().take(500).collect()
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

fn map_integration(error: IntegrationError) -> ApiError {
    match error {
        IntegrationError::NotFound | IntegrationError::Provider(GitHubError::NotFound) => api_error(
            StatusCode::NOT_FOUND,
            "REPOSITORY_NOT_FOUND",
            "repository not found or not accessible",
        ),
        IntegrationError::Provider(GitHubError::PermissionDenied) => api_error(
            StatusCode::FORBIDDEN,
            "GITHUB_PERMISSION_DENIED",
            "GitHub credential does not have access to this repository or required read permissions",
        ),
        IntegrationError::Invalid => api_error(
            StatusCode::BAD_REQUEST,
            "INVALID_REQUEST",
            "invalid GitHub repository reference",
        ),
        IntegrationError::Conflict => api_error(
            StatusCode::CONFLICT,
            "REPOSITORY_ALREADY_LINKED",
            "repository is already linked to this project",
        ),
        IntegrationError::Provider(error) => {
            tracing::warn!(%error, "GitHub provider request failed");
            api_error(
                StatusCode::BAD_GATEWAY,
                "GITHUB_UNAVAILABLE",
                "GitHub metadata could not be fetched",
            )
        }
        other => {
            tracing::error!(error=%other, "repository integration storage error");
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
    fn repository_segments_reject_paths_and_shell_input() {
        for invalid in ["", "../argus", "owner/repo", "repo?x=1", "repo;id"] {
            assert!(matches!(
                validate_segment(invalid, 100),
                Err(IntegrationError::Invalid)
            ));
        }
        assert_eq!(validate_segment("Noah-Bozkurt", 100).unwrap(), "Noah-Bozkurt");
        assert_eq!(validate_segment("argus.rs", 100).unwrap(), "argus.rs");
    }

    #[test]
    fn running_check_takes_precedence_over_completed_results() {
        let summary = summarize_checks(&GitHubCheckRuns {
            total_count: 2,
            check_runs: vec![
                GitHubCheckRun {
                    status: "completed".into(),
                    conclusion: Some("failure".into()),
                },
                GitHubCheckRun {
                    status: "in_progress".into(),
                    conclusion: None,
                },
            ],
        });
        assert_eq!(summary.state, "RUNNING");
    }

    #[test]
    fn completed_failed_check_marks_ci_failed() {
        let summary = summarize_checks(&GitHubCheckRuns {
            total_count: 1,
            check_runs: vec![GitHubCheckRun {
                status: "completed".into(),
                conclusion: Some("failure".into()),
            }],
        });
        assert_eq!(summary.state, "FAILURE");
    }
}
