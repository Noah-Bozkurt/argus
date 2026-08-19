use crate::{ApiError, AppState, api_error, web_identity};
use axum::{
    Json, Router,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    routing::{get, post},
};
use chrono::{DateTime, Duration, Utc};
use reqwest::Url;
use serde::Serialize;
use sqlx::{PgPool, Row};
use std::collections::HashMap;
use uuid::Uuid;

const TLS_FRESHNESS_HOURS: i64 = 24;
const CHECK_HISTORY_PER_SITE: i64 = 20;

#[derive(Debug, Clone)]
pub struct DomainLifecycleStore {
    pool: PgPool,
}

#[derive(Debug, Clone, Serialize)]
pub struct DomainLifecycleView {
    pub domain_id: Uuid,
    pub hostname: String,
    pub expires_at: Option<DateTime<Utc>>,
    pub expiration_status: String,
    pub tls_status: String,
    pub overall_status: String,
    pub days_until_expiry: Option<i32>,
    pub last_evaluated_at: Option<DateTime<Utc>>,
    pub changed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DomainLifecycleEvaluation {
    pub evaluated_domains: usize,
    pub changed_domains: usize,
}

#[derive(Debug, thiserror::Error)]
pub enum DomainLifecycleError {
    #[error("project not found")]
    NotFound,
    #[error(transparent)]
    Sql(#[from] sqlx::Error),
}

#[derive(Debug)]
struct DomainCandidate {
    domain_id: Uuid,
    project_id: Uuid,
    site_id: Option<Uuid>,
    hostname: String,
    expires_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone)]
struct MonitorObservation {
    target_url: String,
    tls_status: String,
    checked_at: DateTime<Utc>,
}

#[derive(Debug)]
struct DerivedState {
    expiration_status: String,
    tls_status: String,
    overall_status: String,
    days_until_expiry: Option<i32>,
}

impl DomainLifecycleStore {
    pub async fn connect(database_url: &str) -> Result<Self, sqlx::Error> {
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(10)
            .connect(database_url)
            .await?;
        Ok(Self { pool })
    }

    pub async fn project_view(
        &self,
        organization_id: Uuid,
        project_id: Uuid,
    ) -> Result<Vec<DomainLifecycleView>, DomainLifecycleError> {
        self.authorize_project(organization_id, project_id).await?;
        let rows = sqlx::query(
            "SELECT d.id AS domain_id,d.hostname,d.expires_at,s.expiration_status,s.tls_status,s.overall_status,s.days_until_expiry,s.last_evaluated_at,s.changed_at FROM domains d LEFT JOIN domain_lifecycle_states s ON s.domain_id=d.id AND s.organization_id=d.organization_id AND s.project_id=d.project_id WHERE d.organization_id=$1 AND d.project_id=$2 ORDER BY d.hostname",
        )
        .bind(organization_id)
        .bind(project_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(|row| DomainLifecycleView {
                domain_id: row.get("domain_id"),
                hostname: row.get("hostname"),
                expires_at: row.get("expires_at"),
                expiration_status: row
                    .get::<Option<String>, _>("expiration_status")
                    .unwrap_or_else(|| "UNKNOWN".into()),
                tls_status: row
                    .get::<Option<String>, _>("tls_status")
                    .unwrap_or_else(|| "UNKNOWN".into()),
                overall_status: row
                    .get::<Option<String>, _>("overall_status")
                    .unwrap_or_else(|| "UNKNOWN".into()),
                days_until_expiry: row.get("days_until_expiry"),
                last_evaluated_at: row.get("last_evaluated_at"),
                changed_at: row.get("changed_at"),
            })
            .collect())
    }

    pub async fn evaluate_organization(
        &self,
        organization_id: Uuid,
    ) -> Result<DomainLifecycleEvaluation, DomainLifecycleError> {
        self.evaluate_scope(organization_id, None).await
    }

    pub async fn evaluate_project(
        &self,
        identity: crate::persistence::WebIdentity,
        project_id: Uuid,
    ) -> Result<DomainLifecycleEvaluation, DomainLifecycleError> {
        self.authorize_project(identity.organization_id, project_id)
            .await?;
        let result = self
            .evaluate_scope(identity.organization_id, Some(project_id))
            .await?;
        sqlx::query(
            "INSERT INTO audit_events(id,organization_id,actor,resource,action,request_id,result,source,timestamp) VALUES($1,$2,$3,$4,'domain.lifecycle.evaluated',$5,'SUCCEEDED','web',NOW())",
        )
        .bind(Uuid::new_v4())
        .bind(identity.organization_id)
        .bind(identity.user_id.to_string())
        .bind(project_id.to_string())
        .bind(Uuid::new_v4().to_string())
        .execute(&self.pool)
        .await?;
        Ok(result)
    }

    async fn evaluate_scope(
        &self,
        organization_id: Uuid,
        project_id: Option<Uuid>,
    ) -> Result<DomainLifecycleEvaluation, DomainLifecycleError> {
        let domain_rows = if let Some(project_id) = project_id {
            sqlx::query(
                "SELECT id AS domain_id,project_id,site_id,hostname,expires_at FROM domains WHERE organization_id=$1 AND project_id=$2 ORDER BY project_id,hostname",
            )
            .bind(organization_id)
            .bind(project_id)
            .fetch_all(&self.pool)
            .await?
        } else {
            sqlx::query(
                "SELECT id AS domain_id,project_id,site_id,hostname,expires_at FROM domains WHERE organization_id=$1 ORDER BY project_id,hostname",
            )
            .bind(organization_id)
            .fetch_all(&self.pool)
            .await?
        };
        let candidates = domain_rows
            .into_iter()
            .map(|row| DomainCandidate {
                domain_id: row.get("domain_id"),
                project_id: row.get("project_id"),
                site_id: row.get("site_id"),
                hostname: row.get("hostname"),
                expires_at: row.get("expires_at"),
            })
            .collect::<Vec<_>>();

        let check_rows = if let Some(project_id) = project_id {
            sqlx::query(
                "SELECT site_id,target_url,tls_status,checked_at FROM (SELECT c.*,ROW_NUMBER() OVER(PARTITION BY site_id ORDER BY checked_at DESC,id DESC) AS rn FROM site_monitor_checks c WHERE organization_id=$1 AND project_id=$2) ranked WHERE rn <= $3 ORDER BY site_id,checked_at DESC",
            )
            .bind(organization_id)
            .bind(project_id)
            .bind(CHECK_HISTORY_PER_SITE)
            .fetch_all(&self.pool)
            .await?
        } else {
            sqlx::query(
                "SELECT site_id,target_url,tls_status,checked_at FROM (SELECT c.*,ROW_NUMBER() OVER(PARTITION BY site_id ORDER BY checked_at DESC,id DESC) AS rn FROM site_monitor_checks c WHERE organization_id=$1) ranked WHERE rn <= $2 ORDER BY site_id,checked_at DESC",
            )
            .bind(organization_id)
            .bind(CHECK_HISTORY_PER_SITE)
            .fetch_all(&self.pool)
            .await?
        };
        let mut observations: HashMap<Uuid, Vec<MonitorObservation>> = HashMap::new();
        for row in check_rows {
            observations
                .entry(row.get("site_id"))
                .or_default()
                .push(MonitorObservation {
                    target_url: row.get("target_url"),
                    tls_status: row.get("tls_status"),
                    checked_at: row.get("checked_at"),
                });
        }

        let now = Utc::now();
        let mut changed_domains = 0_usize;
        for candidate in &candidates {
            let domain_observations = candidate
                .site_id
                .and_then(|site_id| observations.get(&site_id))
                .map(Vec::as_slice)
                .unwrap_or(&[]);
            let state = derive_state(candidate, domain_observations, now);
            if self
                .persist_state(organization_id, candidate, &state)
                .await?
            {
                changed_domains += 1;
            }
        }
        Ok(DomainLifecycleEvaluation {
            evaluated_domains: candidates.len(),
            changed_domains,
        })
    }

    async fn persist_state(
        &self,
        organization_id: Uuid,
        domain: &DomainCandidate,
        state: &DerivedState,
    ) -> Result<bool, DomainLifecycleError> {
        let old = sqlx::query(
            "SELECT expiration_status,tls_status,overall_status,days_until_expiry FROM domain_lifecycle_states WHERE domain_id=$1 AND organization_id=$2 AND project_id=$3",
        )
        .bind(domain.domain_id)
        .bind(organization_id)
        .bind(domain.project_id)
        .fetch_optional(&self.pool)
        .await?;
        let changed = old.as_ref().is_none_or(|row| {
            row.get::<String, _>("expiration_status") != state.expiration_status
                || row.get::<String, _>("tls_status") != state.tls_status
                || row.get::<String, _>("overall_status") != state.overall_status
                || row.get::<Option<i32>, _>("days_until_expiry") != state.days_until_expiry
        });

        let mut tx = self.pool.begin().await?;
        sqlx::query(
            "INSERT INTO domain_lifecycle_states(domain_id,organization_id,project_id,expiration_status,tls_status,overall_status,days_until_expiry,last_evaluated_at,changed_at) VALUES($1,$2,$3,$4,$5,$6,$7,NOW(),NOW()) ON CONFLICT(domain_id) DO UPDATE SET expiration_status=EXCLUDED.expiration_status,tls_status=EXCLUDED.tls_status,overall_status=EXCLUDED.overall_status,days_until_expiry=EXCLUDED.days_until_expiry,last_evaluated_at=NOW(),changed_at=CASE WHEN domain_lifecycle_states.expiration_status IS DISTINCT FROM EXCLUDED.expiration_status OR domain_lifecycle_states.tls_status IS DISTINCT FROM EXCLUDED.tls_status OR domain_lifecycle_states.overall_status IS DISTINCT FROM EXCLUDED.overall_status OR domain_lifecycle_states.days_until_expiry IS DISTINCT FROM EXCLUDED.days_until_expiry THEN NOW() ELSE domain_lifecycle_states.changed_at END",
        )
        .bind(domain.domain_id)
        .bind(organization_id)
        .bind(domain.project_id)
        .bind(&state.expiration_status)
        .bind(&state.tls_status)
        .bind(&state.overall_status)
        .bind(state.days_until_expiry)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            "UPDATE domains SET tls_status=$1 WHERE id=$2 AND organization_id=$3 AND project_id=$4",
        )
        .bind(&state.tls_status)
        .bind(domain.domain_id)
        .bind(organization_id)
        .bind(domain.project_id)
        .execute(&mut *tx)
        .await?;

        if changed {
            let previous = old.map(|row| {
                serde_json::json!({
                    "expiration_status": row.get::<String,_>("expiration_status"),
                    "tls_status": row.get::<String,_>("tls_status"),
                    "overall_status": row.get::<String,_>("overall_status"),
                    "days_until_expiry": row.get::<Option<i32>,_>("days_until_expiry"),
                })
            });
            sqlx::query(
                "INSERT INTO domain_events(id,organization_id,event_type,resource_id,data,occurred_at) VALUES($1,$2,'domain.lifecycle.changed',$3,$4,NOW())",
            )
            .bind(Uuid::new_v4())
            .bind(organization_id)
            .bind(domain.project_id)
            .bind(serde_json::json!({
                "domain_id": domain.domain_id,
                "hostname": domain.hostname,
                "expiration_status": state.expiration_status,
                "tls_status": state.tls_status,
                "overall_status": state.overall_status,
                "days_until_expiry": state.days_until_expiry,
                "previous": previous,
            }))
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(changed)
    }

    async fn authorize_project(
        &self,
        organization_id: Uuid,
        project_id: Uuid,
    ) -> Result<(), DomainLifecycleError> {
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
            Err(DomainLifecycleError::NotFound)
        }
    }
}

fn derive_state(
    domain: &DomainCandidate,
    observations: &[MonitorObservation],
    now: DateTime<Utc>,
) -> DerivedState {
    let (expiration_status, days_until_expiry) = match domain.expires_at {
        None => ("UNKNOWN".to_string(), None),
        Some(expires_at) => {
            let seconds = (expires_at - now).num_seconds();
            let days = if seconds >= 0 {
                ((seconds + 86_399) / 86_400) as i32
            } else {
                -(((-seconds) + 86_399) / 86_400) as i32
            };
            let status = if expires_at <= now {
                "EXPIRED"
            } else if expires_at <= now + Duration::days(7) {
                "CRITICAL"
            } else if expires_at <= now + Duration::days(30) {
                "WARNING"
            } else {
                "OK"
            };
            (status.to_string(), Some(days))
        }
    };

    let tls_status = derive_tls_status(&domain.hostname, observations, now);
    let overall_status =
        if matches!(expiration_status.as_str(), "EXPIRED" | "CRITICAL") || tls_status == "FAILED" {
            "CRITICAL"
        } else if expiration_status == "UNKNOWN" && tls_status == "UNKNOWN" {
            "UNKNOWN"
        } else if expiration_status == "WARNING"
            || expiration_status == "UNKNOWN"
            || matches!(tls_status.as_str(), "STALE" | "UNKNOWN")
        {
            "ATTENTION"
        } else {
            "OK"
        };
    DerivedState {
        expiration_status,
        tls_status,
        overall_status: overall_status.into(),
        days_until_expiry,
    }
}

fn derive_tls_status(
    hostname: &str,
    observations: &[MonitorObservation],
    now: DateTime<Utc>,
) -> String {
    let observation = observations.iter().find(|observation| {
        let Ok(url) = Url::parse(&observation.target_url) else {
            return false;
        };
        url.scheme() == "https"
            && url.host_str().is_some_and(|host| {
                host.trim_end_matches('.')
                    .eq_ignore_ascii_case(hostname.trim_end_matches('.'))
            })
    });
    let Some(observation) = observation else {
        return "UNKNOWN".into();
    };
    if observation.checked_at < now - Duration::hours(TLS_FRESHNESS_HOURS) {
        return "STALE".into();
    }
    if observation.tls_status == "VALID" {
        "VALID".into()
    } else {
        "FAILED".into()
    }
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/projects/:project_id/domain-lifecycle",
            get(get_project_lifecycle),
        )
        .route(
            "/projects/:project_id/domain-lifecycle/evaluate",
            post(evaluate_project_lifecycle),
        )
}

async fn get_project_lifecycle(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(project_id): Path<Uuid>,
) -> Result<Json<Vec<DomainLifecycleView>>, ApiError> {
    let identity = web_identity(&state, &headers).await?;
    Ok(Json(
        state
            .domain_lifecycle
            .project_view(identity.organization_id, project_id)
            .await
            .map_err(map_lifecycle)?,
    ))
}

async fn evaluate_project_lifecycle(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(project_id): Path<Uuid>,
) -> Result<Json<DomainLifecycleEvaluation>, ApiError> {
    let identity = web_identity(&state, &headers).await?;
    Ok(Json(
        state
            .domain_lifecycle
            .evaluate_project(identity, project_id)
            .await
            .map_err(map_lifecycle)?,
    ))
}

fn map_lifecycle(error: DomainLifecycleError) -> ApiError {
    match error {
        DomainLifecycleError::NotFound => api_error(
            StatusCode::NOT_FOUND,
            "PROJECT_NOT_FOUND",
            "project not found",
        ),
        other => {
            tracing::error!(error=%other, "domain lifecycle error");
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

    fn candidate(expires_at: Option<DateTime<Utc>>) -> DomainCandidate {
        DomainCandidate {
            domain_id: Uuid::new_v4(),
            project_id: Uuid::new_v4(),
            site_id: Some(Uuid::new_v4()),
            hostname: "example.com".into(),
            expires_at,
        }
    }

    #[test]
    fn expiration_thresholds_are_conservative() {
        let now = Utc::now();
        assert_eq!(
            derive_state(&candidate(Some(now + Duration::days(5))), &[], now).expiration_status,
            "CRITICAL"
        );
        assert_eq!(
            derive_state(&candidate(Some(now + Duration::days(20))), &[], now).expiration_status,
            "WARNING"
        );
        assert_eq!(
            derive_state(&candidate(Some(now + Duration::days(60))), &[], now).expiration_status,
            "OK"
        );
    }

    #[test]
    fn tls_uses_latest_matching_https_observation() {
        let now = Utc::now();
        let observations = vec![
            MonitorObservation {
                target_url: "https://other.example/".into(),
                tls_status: "VALID".into(),
                checked_at: now,
            },
            MonitorObservation {
                target_url: "https://example.com/health".into(),
                tls_status: "VALID".into(),
                checked_at: now - Duration::hours(1),
            },
        ];
        assert_eq!(
            derive_tls_status("example.com", &observations, now),
            "VALID"
        );
    }

    #[test]
    fn failed_tls_is_critical_and_old_tls_is_stale() {
        let now = Utc::now();
        let failed = vec![MonitorObservation {
            target_url: "https://example.com/".into(),
            tls_status: "FAILED".into(),
            checked_at: now,
        }];
        assert_eq!(
            derive_state(&candidate(None), &failed, now).overall_status,
            "CRITICAL"
        );

        let stale = vec![MonitorObservation {
            target_url: "https://example.com/".into(),
            tls_status: "VALID".into(),
            checked_at: now - Duration::hours(25),
        }];
        assert_eq!(derive_tls_status("example.com", &stale, now), "STALE");
    }
}
