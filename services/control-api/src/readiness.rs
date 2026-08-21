use crate::{ApiError, AppState, api_error, web_identity};
use axum::{
    Json, Router,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    routing::get,
};
use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::{PgPool, Row};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct ReadinessStore {
    pool: PgPool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ReadinessCheck {
    pub key: String,
    pub category: String,
    pub label: String,
    pub status: String,
    pub summary: String,
    pub evidence: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ReadinessAssessment {
    pub project_id: Uuid,
    pub status: String,
    pub checked_at: DateTime<Utc>,
    pub checks: Vec<ReadinessCheck>,
}

#[derive(Debug, thiserror::Error)]
pub enum ReadinessError {
    #[error("project not found")]
    NotFound,
    #[error(transparent)]
    Sql(#[from] sqlx::Error),
    #[error(transparent)]
    Storage(#[from] crate::persistence::StorageError),
    #[error(transparent)]
    Incident(#[from] crate::incidents::IncidentError),
    #[error(transparent)]
    Monitoring(#[from] crate::site_monitoring::SiteMonitoringError),
}

#[derive(Debug)]
struct ServiceTarget {
    id: Uuid,
    name: String,
    environment_id: Option<Uuid>,
    repository_id: Option<Uuid>,
}

impl ReadinessStore {
    pub async fn connect(database_url: &str) -> Result<Self, sqlx::Error> {
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(10)
            .connect(database_url)
            .await?;
        Ok(Self { pool })
    }

    pub async fn assess(
        &self,
        state: &AppState,
        identity: crate::persistence::WebIdentity,
        project_id: Uuid,
    ) -> Result<ReadinessAssessment, ReadinessError> {
        let exists: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM projects WHERE id=$1 AND organization_id=$2)",
        )
        .bind(project_id)
        .bind(identity.organization_id)
        .fetch_one(&self.pool)
        .await?;
        if !exists {
            return Err(ReadinessError::NotFound);
        }

        let environment_rows = sqlx::query(
            "SELECT id,name,type AS environment_type,is_protected FROM environments WHERE organization_id=$1 AND project_id=$2 ORDER BY sort_order,name",
        )
        .bind(identity.organization_id)
        .bind(project_id)
        .fetch_all(&self.pool)
        .await?;
        let production_environments: HashMap<Uuid, String> = environment_rows
            .iter()
            .filter(|row| row.get::<String, _>("environment_type") == "production")
            .map(|row| (row.get("id"), row.get("name")))
            .collect();
        let production_ids: HashSet<Uuid> = production_environments.keys().copied().collect();

        let service_rows = sqlx::query(
            "SELECT id,name,environment_id,repository_id FROM services WHERE organization_id=$1 AND project_id=$2 AND lifecycle_status='ACTIVE' ORDER BY name",
        )
        .bind(identity.organization_id)
        .bind(project_id)
        .fetch_all(&self.pool)
        .await?;
        let services: Vec<ServiceTarget> = service_rows
            .into_iter()
            .map(|row| ServiceTarget {
                id: row.get("id"),
                name: row.get("name"),
                environment_id: row.get("environment_id"),
                repository_id: row.get("repository_id"),
            })
            .collect();
        let production_services: Vec<&ServiceTarget> = services
            .iter()
            .filter(|service| {
                service
                    .environment_id
                    .is_some_and(|id| production_ids.contains(&id))
            })
            .collect();

        let mut checks = Vec::new();
        checks.push(check_production_environment(
            &production_environments,
            &services,
        ));
        checks.push(
            self.check_deployments(identity.organization_id, project_id, &production_services)
                .await?,
        );
        checks.push(
            self.check_repository_ci(identity.organization_id, project_id, &production_services)
                .await?,
        );

        let monitoring = state
            .site_monitoring
            .project_view(identity.organization_id, project_id)
            .await?;
        let site_rows = sqlx::query(
            "SELECT id,name,lifecycle_status FROM sites WHERE organization_id=$1 AND project_id=$2 ORDER BY name",
        )
        .bind(identity.organization_id)
        .bind(project_id)
        .fetch_all(&self.pool)
        .await?;
        checks.push(check_site_monitoring(&site_rows, &monitoring));

        let servers = state.storage.list_servers(identity).await?;
        let project_servers: Vec<_> = servers
            .iter()
            .filter(|server| server.project_id == project_id)
            .collect();
        let production_servers: Vec<_> = project_servers
            .into_iter()
            .filter(|server| production_ids.contains(&server.environment_id))
            .collect();
        checks.push(check_server_health(&production_servers));
        checks.push(check_backups(&production_servers));
        checks.push(check_security(&production_servers));
        checks.push(check_updates(&production_servers));

        let incidents = state
            .incidents
            .list(identity.organization_id, project_id)
            .await?;
        checks.push(check_incidents(&incidents));
        checks.push(
            self.check_latest_release(identity.organization_id, project_id)
                .await?,
        );

        let status = if checks.iter().any(|check| check.status == "BLOCKED") {
            "BLOCKED"
        } else if checks.iter().any(|check| check.status == "WARN") {
            "ATTENTION"
        } else {
            "READY"
        };
        Ok(ReadinessAssessment {
            project_id,
            status: status.into(),
            checked_at: Utc::now(),
            checks,
        })
    }

    async fn check_deployments(
        &self,
        organization_id: Uuid,
        project_id: Uuid,
        services: &[&ServiceTarget],
    ) -> Result<ReadinessCheck, sqlx::Error> {
        if services.is_empty() {
            return Ok(skipped(
                "production-deployments",
                "DEPLOYMENT",
                "Production deployments",
                "No active Services are assigned to a production Environment.",
            ));
        }
        let mut missing = Vec::new();
        let mut unsuccessful = Vec::new();
        let mut evidence = Vec::new();
        for service in services {
            let environment_id = service
                .environment_id
                .expect("production service environment");
            let deployment = sqlx::query(
                "SELECT status,source_version,source_commit_sha,created_at FROM deployments WHERE organization_id=$1 AND project_id=$2 AND service_id=$3 AND environment_id=$4 ORDER BY created_at DESC LIMIT 1",
            )
            .bind(organization_id)
            .bind(project_id)
            .bind(service.id)
            .bind(environment_id)
            .fetch_optional(&self.pool)
            .await?;
            match deployment {
                None => missing.push(service.name.clone()),
                Some(row) => {
                    let status: String = row.get("status");
                    let source_version: Option<String> = row.get("source_version");
                    let source_commit: Option<String> = row.get("source_commit_sha");
                    let source = source_version
                        .or_else(|| source_commit.map(|value| value.chars().take(12).collect()))
                        .unwrap_or_else(|| "unspecified source".into());
                    evidence.push(format!("{}: {} ({})", service.name, status, source));
                    if status != "SUCCEEDED" {
                        unsuccessful.push(format!("{} ({})", service.name, status));
                    }
                }
            }
        }
        if !missing.is_empty() || !unsuccessful.is_empty() {
            let mut reasons = Vec::new();
            if !missing.is_empty() {
                reasons.push(format!("missing deployment: {}", missing.join(", ")));
            }
            if !unsuccessful.is_empty() {
                reasons.push(format!(
                    "latest deployment not successful: {}",
                    unsuccessful.join(", ")
                ));
            }
            return Ok(blocked(
                "production-deployments",
                "DEPLOYMENT",
                "Production deployments",
                &reasons.join("; "),
                evidence,
            ));
        }
        Ok(pass(
            "production-deployments",
            "DEPLOYMENT",
            "Production deployments",
            "Every active production Service has a successful latest deployment.",
            evidence,
        ))
    }

    async fn check_repository_ci(
        &self,
        organization_id: Uuid,
        project_id: Uuid,
        services: &[&ServiceTarget],
    ) -> Result<ReadinessCheck, sqlx::Error> {
        let repository_ids: HashSet<Uuid> = services
            .iter()
            .filter_map(|service| service.repository_id)
            .collect();
        if repository_ids.is_empty() {
            return Ok(warn(
                "repository-ci",
                "SOURCE",
                "Repository CI",
                "No repository is linked to active production Services.",
                Vec::new(),
            ));
        }
        let rows = sqlx::query(
            "SELECT id,owner,name,sync_status,snapshot FROM project_repositories WHERE organization_id=$1 AND project_id=$2 AND id = ANY($3)",
        )
        .bind(organization_id)
        .bind(project_id)
        .bind(repository_ids.iter().copied().collect::<Vec<_>>())
        .fetch_all(&self.pool)
        .await?;
        let mut evidence = Vec::new();
        let mut failures = Vec::new();
        let mut warnings = Vec::new();
        for row in rows {
            let owner: String = row.get("owner");
            let name: String = row.get("name");
            let sync_status: String = row.get("sync_status");
            let snapshot: serde_json::Value = row.get("snapshot");
            let ci = snapshot
                .pointer("/ci/state")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("UNAVAILABLE");
            evidence.push(format!("{owner}/{name}: sync {sync_status}, CI {ci}"));
            if sync_status == "ERROR" || ci == "FAILURE" {
                failures.push(format!("{owner}/{name}"));
            } else if sync_status != "SYNCED" || !matches!(ci, "SUCCESS") {
                warnings.push(format!("{owner}/{name}: {ci}"));
            }
        }
        if !failures.is_empty() {
            Ok(blocked(
                "repository-ci",
                "SOURCE",
                "Repository CI",
                &format!("CI or repository sync failure: {}", failures.join(", ")),
                evidence,
            ))
        } else if !warnings.is_empty() {
            Ok(warn(
                "repository-ci",
                "SOURCE",
                "Repository CI",
                &format!("CI is not fully green/available: {}", warnings.join(", ")),
                evidence,
            ))
        } else {
            Ok(pass(
                "repository-ci",
                "SOURCE",
                "Repository CI",
                "Linked production repositories are synced and CI is successful.",
                evidence,
            ))
        }
    }

    async fn check_latest_release(
        &self,
        organization_id: Uuid,
        project_id: Uuid,
    ) -> Result<ReadinessCheck, sqlx::Error> {
        let row = sqlx::query(
            "SELECT version,name,status,created_at FROM releases WHERE organization_id=$1 AND project_id=$2 ORDER BY created_at DESC LIMIT 1",
        )
        .bind(organization_id)
        .bind(project_id)
        .fetch_optional(&self.pool)
        .await?;
        let Some(row) = row else {
            return Ok(warn(
                "release-record",
                "RELEASE",
                "Release record",
                "No Release record exists yet.",
                Vec::new(),
            ));
        };
        let version: String = row.get("version");
        let name: String = row.get("name");
        let status: String = row.get("status");
        let evidence = vec![format!("{name} ({version}): {status}")];
        match status.as_str() {
            "READY" | "RELEASED" => Ok(pass(
                "release-record",
                "RELEASE",
                "Release record",
                "Latest Release is ready or released.",
                evidence,
            )),
            "FAILED" | "ROLLED_BACK" => Ok(blocked(
                "release-record",
                "RELEASE",
                "Release record",
                "Latest Release is failed or rolled back.",
                evidence,
            )),
            _ => Ok(warn(
                "release-record",
                "RELEASE",
                "Release record",
                "Latest Release is still a draft.",
                evidence,
            )),
        }
    }
}

pub fn router() -> Router<AppState> {
    Router::new().route(
        "/projects/:project_id/readiness",
        get(get_project_readiness),
    )
}

async fn get_project_readiness(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(project_id): Path<Uuid>,
) -> Result<Json<ReadinessAssessment>, ApiError> {
    let identity = web_identity(&state, &headers).await?;
    Ok(Json(
        state
            .readiness
            .assess(&state, identity, project_id)
            .await
            .map_err(map_readiness)?,
    ))
}

fn check_production_environment(
    production: &HashMap<Uuid, String>,
    services: &[ServiceTarget],
) -> ReadinessCheck {
    if production.is_empty() {
        return blocked(
            "production-environment",
            "ENVIRONMENT",
            "Production environment",
            "No production Environment exists.",
            Vec::new(),
        );
    }
    let unassigned: Vec<String> = services
        .iter()
        .filter(|service| service.environment_id.is_none())
        .map(|service| service.name.clone())
        .collect();
    if !unassigned.is_empty() {
        return warn(
            "production-environment",
            "ENVIRONMENT",
            "Production environment",
            &format!(
                "Active Services without an Environment: {}",
                unassigned.join(", ")
            ),
            production.values().cloned().collect(),
        );
    }
    pass(
        "production-environment",
        "ENVIRONMENT",
        "Production environment",
        "Production Environment is defined.",
        production.values().cloned().collect(),
    )
}

fn check_site_monitoring(
    site_rows: &[sqlx::postgres::PgRow],
    monitoring: &crate::site_monitoring::ProjectMonitoringView,
) -> ReadinessCheck {
    let active_sites: HashMap<Uuid, String> = site_rows
        .iter()
        .filter(|row| row.get::<String, _>("lifecycle_status") == "ACTIVE")
        .map(|row| (row.get("id"), row.get("name")))
        .collect();
    if active_sites.is_empty() {
        return skipped(
            "site-monitoring",
            "RELIABILITY",
            "Site monitoring",
            "No active Sites exist.",
        );
    }
    let monitors: HashMap<Uuid, _> = monitoring
        .monitors
        .iter()
        .map(|monitor| (monitor.site_id, monitor))
        .collect();
    let mut missing = Vec::new();
    let mut failures = Vec::new();
    let mut degraded = Vec::new();
    let mut evidence = Vec::new();
    for (site_id, name) in active_sites {
        let Some(monitor) = monitors.get(&site_id) else {
            missing.push(name);
            continue;
        };
        if monitor.config.is_none() || monitor.checks.is_empty() {
            missing.push(name);
            continue;
        }
        let latest = &monitor.checks[0];
        evidence.push(format!("{}: {}", name, latest.overall_status));
        match latest.overall_status.as_str() {
            "HEALTHY" => {}
            "DEGRADED" => degraded.push(name),
            _ => failures.push(name),
        }
    }
    if !failures.is_empty() {
        blocked(
            "site-monitoring",
            "RELIABILITY",
            "Site monitoring",
            &format!("Active Sites are down/error: {}", failures.join(", ")),
            evidence,
        )
    } else if !missing.is_empty() || !degraded.is_empty() {
        let mut reasons = Vec::new();
        if !missing.is_empty() {
            reasons.push(format!("no check: {}", missing.join(", ")));
        }
        if !degraded.is_empty() {
            reasons.push(format!("degraded: {}", degraded.join(", ")));
        }
        warn(
            "site-monitoring",
            "RELIABILITY",
            "Site monitoring",
            &reasons.join("; "),
            evidence,
        )
    } else {
        pass(
            "site-monitoring",
            "RELIABILITY",
            "Site monitoring",
            "All active Sites have a healthy latest check.",
            evidence,
        )
    }
}

fn check_server_health(servers: &[&crate::persistence::ServerView]) -> ReadinessCheck {
    if servers.is_empty() {
        return skipped(
            "production-servers",
            "INFRASTRUCTURE",
            "Production servers",
            "This project has no production Servers; server checks are not applicable.",
        );
    }
    let bad: Vec<String> = servers
        .iter()
        .filter(|server| !server.online || server.snapshot.is_none())
        .map(|server| server.hostname.clone())
        .collect();
    let evidence = servers
        .iter()
        .map(|server| {
            format!(
                "{}: {}{}",
                server.hostname,
                if server.online { "online" } else { "offline" },
                if server.snapshot.is_some() {
                    ""
                } else {
                    ", no snapshot"
                }
            )
        })
        .collect();
    if bad.is_empty() {
        pass(
            "production-servers",
            "INFRASTRUCTURE",
            "Production servers",
            "Production Servers are online with current Agent snapshots.",
            evidence,
        )
    } else {
        blocked(
            "production-servers",
            "INFRASTRUCTURE",
            "Production servers",
            &format!(
                "Production Servers unavailable/incomplete: {}",
                bad.join(", ")
            ),
            evidence,
        )
    }
}

fn check_backups(servers: &[&crate::persistence::ServerView]) -> ReadinessCheck {
    if servers.is_empty() {
        return skipped(
            "verified-backup",
            "RELIABILITY",
            "Verified backup",
            "No production Servers require Argus system-config backups.",
        );
    }
    let mut missing = Vec::new();
    let mut evidence = Vec::new();
    for server in servers {
        let verified = server.snapshot.as_ref().is_some_and(|snapshot| {
            snapshot
                .backups
                .artifacts
                .iter()
                .any(|artifact| artifact.verified)
        });
        evidence.push(format!("{}: verified backup {}", server.hostname, verified));
        if !verified {
            missing.push(server.hostname.clone());
        }
    }
    if missing.is_empty() {
        pass(
            "verified-backup",
            "RELIABILITY",
            "Verified backup",
            "Every production Server has at least one verified backup artifact.",
            evidence,
        )
    } else {
        blocked(
            "verified-backup",
            "RELIABILITY",
            "Verified backup",
            &format!("Missing verified backup: {}", missing.join(", ")),
            evidence,
        )
    }
}

fn check_security(servers: &[&crate::persistence::ServerView]) -> ReadinessCheck {
    if servers.is_empty() {
        return skipped(
            "security-findings",
            "SECURITY",
            "Security findings",
            "No production Servers require Agent security checks.",
        );
    }
    let mut blockers = Vec::new();
    let mut warnings = Vec::new();
    let mut evidence = Vec::new();
    for server in servers {
        let Some(snapshot) = &server.snapshot else {
            warnings.push(format!("{}: no snapshot", server.hostname));
            continue;
        };
        if !snapshot.security.available {
            warnings.push(format!("{}: security unavailable", server.hostname));
            continue;
        }
        for finding in &snapshot.security.findings {
            evidence.push(format!(
                "{}: {} {}",
                server.hostname, finding.severity, finding.code
            ));
            match finding.severity.to_uppercase().as_str() {
                "CRITICAL" | "HIGH" => {
                    blockers.push(format!("{}: {}", server.hostname, finding.code))
                }
                "MEDIUM" => warnings.push(format!("{}: {}", server.hostname, finding.code)),
                _ => {}
            }
        }
    }
    if !blockers.is_empty() {
        blocked(
            "security-findings",
            "SECURITY",
            "Security findings",
            &format!("High/Critical findings remain: {}", blockers.join(", ")),
            evidence,
        )
    } else if !warnings.is_empty() {
        warn(
            "security-findings",
            "SECURITY",
            "Security findings",
            &format!("Security needs attention: {}", warnings.join(", ")),
            evidence,
        )
    } else {
        pass(
            "security-findings",
            "SECURITY",
            "Security findings",
            "No High/Critical production Server findings are reported.",
            evidence,
        )
    }
}

fn check_updates(servers: &[&crate::persistence::ServerView]) -> ReadinessCheck {
    if servers.is_empty() {
        return skipped(
            "pending-updates",
            "SECURITY",
            "Pending updates",
            "No production Servers require package-update checks.",
        );
    }
    let mut attention = Vec::new();
    let mut evidence = Vec::new();
    for server in servers {
        let Some(snapshot) = &server.snapshot else {
            continue;
        };
        evidence.push(format!(
            "{}: {} pending, reboot {}",
            server.hostname, snapshot.updates.pending_updates, snapshot.updates.reboot_required
        ));
        if snapshot.updates.pending_updates > 0 || snapshot.updates.reboot_required {
            attention.push(server.hostname.clone());
        }
    }
    if attention.is_empty() {
        pass(
            "pending-updates",
            "SECURITY",
            "Pending updates",
            "No production Server package updates/reboots are pending.",
            evidence,
        )
    } else {
        warn(
            "pending-updates",
            "SECURITY",
            "Pending updates",
            &format!("Updates or reboot pending: {}", attention.join(", ")),
            evidence,
        )
    }
}

fn check_incidents(incidents: &[crate::incidents::IncidentSummary]) -> ReadinessCheck {
    let active: Vec<_> = incidents
        .iter()
        .filter(|incident| incident.status != "RESOLVED")
        .collect();
    if active.is_empty() {
        return pass(
            "open-incidents",
            "OPERATIONS",
            "Open incidents",
            "No unresolved Incidents.",
            Vec::new(),
        );
    }
    let evidence: Vec<String> = active
        .iter()
        .map(|incident| {
            format!(
                "{}: {} {}",
                incident.title, incident.severity, incident.status
            )
        })
        .collect();
    if active
        .iter()
        .any(|incident| matches!(incident.severity.as_str(), "MAJOR" | "CRITICAL"))
    {
        blocked(
            "open-incidents",
            "OPERATIONS",
            "Open incidents",
            "Major/Critical unresolved Incidents exist.",
            evidence,
        )
    } else {
        warn(
            "open-incidents",
            "OPERATIONS",
            "Open incidents",
            "Minor unresolved Incidents exist.",
            evidence,
        )
    }
}

fn pass(
    key: &str,
    category: &str,
    label: &str,
    summary: &str,
    evidence: Vec<String>,
) -> ReadinessCheck {
    check(key, category, label, "PASS", summary, evidence)
}
fn warn(
    key: &str,
    category: &str,
    label: &str,
    summary: &str,
    evidence: Vec<String>,
) -> ReadinessCheck {
    check(key, category, label, "WARN", summary, evidence)
}
fn blocked(
    key: &str,
    category: &str,
    label: &str,
    summary: &str,
    evidence: Vec<String>,
) -> ReadinessCheck {
    check(key, category, label, "BLOCKED", summary, evidence)
}
fn skipped(key: &str, category: &str, label: &str, summary: &str) -> ReadinessCheck {
    check(key, category, label, "SKIPPED", summary, Vec::new())
}
fn check(
    key: &str,
    category: &str,
    label: &str,
    status: &str,
    summary: &str,
    evidence: Vec<String>,
) -> ReadinessCheck {
    ReadinessCheck {
        key: key.into(),
        category: category.into(),
        label: label.into(),
        status: status.into(),
        summary: summary.into(),
        evidence,
    }
}

fn map_readiness(error: ReadinessError) -> ApiError {
    match error {
        ReadinessError::NotFound => api_error(
            StatusCode::NOT_FOUND,
            "PROJECT_NOT_FOUND",
            "project not found",
        ),
        other => {
            tracing::error!(error=%other, "readiness assessment error");
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
    fn blocked_check_wins_over_warnings() {
        let checks = [
            warn("a", "X", "A", "warn", vec![]),
            blocked("b", "X", "B", "blocked", vec![]),
        ];
        let status = if checks.iter().any(|check| check.status == "BLOCKED") {
            "BLOCKED"
        } else if checks.iter().any(|check| check.status == "WARN") {
            "ATTENTION"
        } else {
            "READY"
        };
        assert_eq!(status, "BLOCKED");
    }
}
