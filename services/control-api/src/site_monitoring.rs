use crate::{ApiError, AppState, api_error, web_identity};
use axum::{
    Json, Router,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    routing::{get, post},
};
use chrono::{DateTime, Utc};
use reqwest::{Client, Url, redirect::Policy};
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};
use std::{
    collections::{BTreeSet, HashMap},
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr},
    time::{Duration, Instant},
};
use tokio::net::lookup_host;
use uuid::Uuid;

const CHECK_HISTORY_PER_SITE: i64 = 20;

#[derive(Debug, Clone)]
pub struct SiteMonitoringStore {
    pool: PgPool,
}

#[derive(Debug, Clone, Serialize)]
pub struct SiteMonitorConfig {
    pub id: Uuid,
    pub site_id: Uuid,
    pub target_url: String,
    pub check_robots: bool,
    pub check_sitemap: bool,
    pub timeout_seconds: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SiteMonitorCheck {
    pub id: Uuid,
    pub site_id: Uuid,
    pub config_id: Uuid,
    pub overall_status: String,
    pub target_url: String,
    pub resolved_ips: Vec<String>,
    pub dns_ok: bool,
    pub http_status: Option<i32>,
    pub http_latency_ms: Option<i64>,
    pub tls_status: String,
    pub robots_status: Option<i32>,
    pub sitemap_status: Option<i32>,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
    pub checked_by: Uuid,
    pub checked_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SiteMonitorView {
    pub site_id: Uuid,
    pub config: Option<SiteMonitorConfig>,
    pub checks: Vec<SiteMonitorCheck>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProjectMonitoringView {
    pub monitors: Vec<SiteMonitorView>,
}

#[derive(Debug, Deserialize)]
pub struct SaveMonitorConfigRequest {
    pub target_url: String,
    #[serde(default = "default_true")]
    pub check_robots: bool,
    #[serde(default = "default_true")]
    pub check_sitemap: bool,
    #[serde(default = "default_timeout")]
    pub timeout_seconds: i32,
}

#[derive(Debug, thiserror::Error)]
pub enum SiteMonitoringError {
    #[error("site or monitor not found")]
    NotFound,
    #[error("invalid monitor request")]
    Invalid,
    #[error(transparent)]
    Sql(#[from] sqlx::Error),
}

#[derive(Debug)]
struct ProbeResult {
    overall_status: String,
    resolved_ips: Vec<String>,
    dns_ok: bool,
    http_status: Option<i32>,
    http_latency_ms: Option<i64>,
    tls_status: String,
    robots_status: Option<i32>,
    sitemap_status: Option<i32>,
    error_code: Option<String>,
    error_message: Option<String>,
    target_host: String,
}

impl SiteMonitoringStore {
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
    ) -> Result<(), SiteMonitoringError> {
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
            Err(SiteMonitoringError::NotFound)
        }
    }

    pub async fn project_view(
        &self,
        organization_id: Uuid,
        project_id: Uuid,
    ) -> Result<ProjectMonitoringView, SiteMonitoringError> {
        self.authorize_project(organization_id, project_id).await?;
        let site_ids: Vec<Uuid> = sqlx::query_scalar(
            "SELECT id FROM sites WHERE organization_id=$1 AND project_id=$2 ORDER BY created_at",
        )
        .bind(organization_id)
        .bind(project_id)
        .fetch_all(&self.pool)
        .await?;
        let config_rows = sqlx::query(
            "SELECT id,site_id,target_url,check_robots,check_sitemap,timeout_seconds,created_at,updated_at FROM site_monitor_configs WHERE organization_id=$1 AND project_id=$2",
        )
        .bind(organization_id)
        .bind(project_id)
        .fetch_all(&self.pool)
        .await?;
        let mut configs = HashMap::new();
        for row in config_rows {
            let config = config_from_row(row)?;
            configs.insert(config.site_id, config);
        }
        let check_rows = sqlx::query(
            "SELECT id,site_id,config_id,overall_status,target_url,resolved_ips,dns_ok,http_status,http_latency_ms,tls_status,robots_status,sitemap_status,error_code,error_message,checked_by,checked_at FROM (SELECT c.*,ROW_NUMBER() OVER(PARTITION BY site_id ORDER BY checked_at DESC) AS rn FROM site_monitor_checks c WHERE organization_id=$1 AND project_id=$2) ranked WHERE rn <= $3 ORDER BY site_id,checked_at DESC",
        )
        .bind(organization_id)
        .bind(project_id)
        .bind(CHECK_HISTORY_PER_SITE)
        .fetch_all(&self.pool)
        .await?;
        let mut checks: HashMap<Uuid, Vec<SiteMonitorCheck>> = HashMap::new();
        for row in check_rows {
            let check = check_from_row(row)?;
            checks.entry(check.site_id).or_default().push(check);
        }
        Ok(ProjectMonitoringView {
            monitors: site_ids
                .into_iter()
                .map(|site_id| SiteMonitorView {
                    site_id,
                    config: configs.remove(&site_id),
                    checks: checks.remove(&site_id).unwrap_or_default(),
                })
                .collect(),
        })
    }

    pub async fn save_config(
        &self,
        identity: crate::persistence::WebIdentity,
        project_id: Uuid,
        site_id: Uuid,
        request: SaveMonitorConfigRequest,
    ) -> Result<SiteMonitorConfig, SiteMonitoringError> {
        self.authorize_project(identity.organization_id, project_id)
            .await?;
        if !(2..=30).contains(&request.timeout_seconds) {
            return Err(SiteMonitoringError::Invalid);
        }
        let target = normalize_target_url(&request.target_url)?;
        self.ensure_target_belongs_to_site(identity.organization_id, project_id, site_id, &target)
            .await?;
        let id = Uuid::new_v4();
        let mut tx = self.pool.begin().await?;
        sqlx::query(
            "INSERT INTO site_monitor_configs(id,organization_id,project_id,site_id,target_url,check_robots,check_sitemap,timeout_seconds,created_at,updated_at) VALUES($1,$2,$3,$4,$5,$6,$7,$8,NOW(),NOW()) ON CONFLICT(site_id) DO UPDATE SET target_url=EXCLUDED.target_url,check_robots=EXCLUDED.check_robots,check_sitemap=EXCLUDED.check_sitemap,timeout_seconds=EXCLUDED.timeout_seconds,updated_at=NOW()",
        )
        .bind(id)
        .bind(identity.organization_id)
        .bind(project_id)
        .bind(site_id)
        .bind(target.as_str())
        .bind(request.check_robots)
        .bind(request.check_sitemap)
        .bind(request.timeout_seconds)
        .execute(&mut *tx)
        .await?;
        touch_project(&mut tx, identity.organization_id, project_id).await?;
        audit_event(
            &mut tx,
            identity,
            project_id,
            "site.monitor.updated",
            serde_json::json!({"site_id":site_id,"target_url":target.as_str()}),
        )
        .await?;
        tx.commit().await?;
        self.get_config(identity.organization_id, project_id, site_id)
            .await
    }

    pub async fn run_check(
        &self,
        identity: crate::persistence::WebIdentity,
        project_id: Uuid,
        site_id: Uuid,
    ) -> Result<SiteMonitorCheck, SiteMonitoringError> {
        self.authorize_project(identity.organization_id, project_id)
            .await?;
        let config = self
            .get_config(identity.organization_id, project_id, site_id)
            .await?;
        let target = normalize_target_url(&config.target_url)?;
        self.ensure_target_belongs_to_site(identity.organization_id, project_id, site_id, &target)
            .await?;

        let result = probe(&config, &target).await;
        let id = Uuid::new_v4();
        let mut tx = self.pool.begin().await?;
        sqlx::query(
            "INSERT INTO site_monitor_checks(id,organization_id,project_id,site_id,config_id,overall_status,target_url,resolved_ips,dns_ok,http_status,http_latency_ms,tls_status,robots_status,sitemap_status,error_code,error_message,checked_by,checked_at) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17,NOW())",
        )
        .bind(id)
        .bind(identity.organization_id)
        .bind(project_id)
        .bind(site_id)
        .bind(config.id)
        .bind(&result.overall_status)
        .bind(&config.target_url)
        .bind(serde_json::json!(result.resolved_ips))
        .bind(result.dns_ok)
        .bind(result.http_status)
        .bind(result.http_latency_ms)
        .bind(&result.tls_status)
        .bind(result.robots_status)
        .bind(result.sitemap_status)
        .bind(&result.error_code)
        .bind(&result.error_message)
        .bind(identity.user_id)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            "UPDATE sites SET health_status=$1,updated_at=NOW() WHERE id=$2 AND organization_id=$3 AND project_id=$4",
        )
        .bind(&result.overall_status)
        .bind(site_id)
        .bind(identity.organization_id)
        .bind(project_id)
        .execute(&mut *tx)
        .await?;
        if result.tls_status == "VALID" {
            sqlx::query(
                "UPDATE domains SET tls_status='VALID',updated_at=NOW() WHERE organization_id=$1 AND project_id=$2 AND site_id=$3 AND hostname=$4",
            )
            .bind(identity.organization_id)
            .bind(project_id)
            .bind(site_id)
            .bind(&result.target_host)
            .execute(&mut *tx)
            .await?;
        }
        touch_project(&mut tx, identity.organization_id, project_id).await?;
        audit_event(
            &mut tx,
            identity,
            project_id,
            "site.check.completed",
            serde_json::json!({"site_id":site_id,"check_id":id,"status":result.overall_status}),
        )
        .await?;
        tx.commit().await?;
        self.get_check(identity.organization_id, project_id, id)
            .await
    }

    async fn get_config(
        &self,
        organization_id: Uuid,
        project_id: Uuid,
        site_id: Uuid,
    ) -> Result<SiteMonitorConfig, SiteMonitoringError> {
        let row = sqlx::query(
            "SELECT id,site_id,target_url,check_robots,check_sitemap,timeout_seconds,created_at,updated_at FROM site_monitor_configs WHERE organization_id=$1 AND project_id=$2 AND site_id=$3",
        )
        .bind(organization_id)
        .bind(project_id)
        .bind(site_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(SiteMonitoringError::NotFound)?;
        config_from_row(row)
    }

    async fn get_check(
        &self,
        organization_id: Uuid,
        project_id: Uuid,
        check_id: Uuid,
    ) -> Result<SiteMonitorCheck, SiteMonitoringError> {
        let row = sqlx::query(
            "SELECT id,site_id,config_id,overall_status,target_url,resolved_ips,dns_ok,http_status,http_latency_ms,tls_status,robots_status,sitemap_status,error_code,error_message,checked_by,checked_at FROM site_monitor_checks WHERE id=$1 AND organization_id=$2 AND project_id=$3",
        )
        .bind(check_id)
        .bind(organization_id)
        .bind(project_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(SiteMonitoringError::NotFound)?;
        check_from_row(row)
    }

    async fn ensure_target_belongs_to_site(
        &self,
        organization_id: Uuid,
        project_id: Uuid,
        site_id: Uuid,
        target: &Url,
    ) -> Result<(), SiteMonitoringError> {
        let canonical_url: Option<String> = sqlx::query_scalar(
            "SELECT canonical_url FROM sites WHERE id=$1 AND organization_id=$2 AND project_id=$3",
        )
        .bind(site_id)
        .bind(organization_id)
        .bind(project_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(SiteMonitoringError::NotFound)?;
        let target_host = target
            .host_str()
            .ok_or(SiteMonitoringError::Invalid)?
            .trim_end_matches('.')
            .to_ascii_lowercase();
        let mut allowed = BTreeSet::new();
        if let Some(canonical_url) = canonical_url {
            if let Ok(url) = Url::parse(&canonical_url) {
                if let Some(host) = url.host_str() {
                    allowed.insert(host.trim_end_matches('.').to_ascii_lowercase());
                }
            }
        }
        let domains: Vec<String> = sqlx::query_scalar(
            "SELECT hostname FROM domains WHERE organization_id=$1 AND project_id=$2 AND site_id=$3",
        )
        .bind(organization_id)
        .bind(project_id)
        .bind(site_id)
        .fetch_all(&self.pool)
        .await?;
        allowed.extend(domains.into_iter().map(|host| host.to_ascii_lowercase()));
        if allowed.contains(&target_host) {
            Ok(())
        } else {
            Err(SiteMonitoringError::Invalid)
        }
    }
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/projects/:project_id/site-monitoring",
            get(get_project_monitoring),
        )
        .route(
            "/projects/:project_id/sites/:site_id/monitor",
            axum::routing::put(save_monitor_config),
        )
        .route(
            "/projects/:project_id/sites/:site_id/monitor/check",
            post(run_monitor_check),
        )
}

async fn get_project_monitoring(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(project_id): Path<Uuid>,
) -> Result<Json<ProjectMonitoringView>, ApiError> {
    let identity = web_identity(&state, &headers).await?;
    Ok(Json(
        state
            .site_monitoring
            .project_view(identity.organization_id, project_id)
            .await
            .map_err(map_monitoring)?,
    ))
}

async fn save_monitor_config(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((project_id, site_id)): Path<(Uuid, Uuid)>,
    Json(request): Json<SaveMonitorConfigRequest>,
) -> Result<Json<SiteMonitorConfig>, ApiError> {
    let identity = web_identity(&state, &headers).await?;
    Ok(Json(
        state
            .site_monitoring
            .save_config(identity, project_id, site_id, request)
            .await
            .map_err(map_monitoring)?,
    ))
}

async fn run_monitor_check(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((project_id, site_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<SiteMonitorCheck>, ApiError> {
    let identity = web_identity(&state, &headers).await?;
    Ok(Json(
        state
            .site_monitoring
            .run_check(identity, project_id, site_id)
            .await
            .map_err(map_monitoring)?,
    ))
}

async fn probe(config: &SiteMonitorConfig, target: &Url) -> ProbeResult {
    let host = target.host_str().unwrap_or_default().to_ascii_lowercase();
    let scheme = target.scheme();
    let port = target.port_or_known_default().unwrap_or(0);
    let mut result = ProbeResult {
        overall_status: "ERROR".into(),
        resolved_ips: Vec::new(),
        dns_ok: false,
        http_status: None,
        http_latency_ms: None,
        tls_status: if scheme == "https" {
            "FAILED".into()
        } else {
            "NOT_APPLICABLE".into()
        },
        robots_status: None,
        sitemap_status: None,
        error_code: None,
        error_message: None,
        target_host: host.clone(),
    };

    let addresses = match lookup_host((host.as_str(), port)).await {
        Ok(addresses) => addresses.collect::<Vec<_>>(),
        Err(error) => {
            result.overall_status = "DOWN".into();
            result.error_code = Some("DNS_FAILED".into());
            result.error_message = Some(truncate_error(&error.to_string()));
            return result;
        }
    };
    let mut ips = BTreeSet::new();
    for address in addresses {
        ips.insert(address.ip());
    }
    if ips.is_empty() {
        result.overall_status = "DOWN".into();
        result.error_code = Some("DNS_EMPTY".into());
        result.error_message = Some("DNS returned no addresses".into());
        return result;
    }
    result.dns_ok = true;
    result.resolved_ips = ips.iter().map(ToString::to_string).collect();
    if ips.iter().any(|ip| !is_public_ip(*ip)) {
        result.error_code = Some("TARGET_BLOCKED".into());
        result.error_message = Some("target resolved to a non-public address".into());
        return result;
    }
    let pinned_ip = *ips.iter().next().expect("non-empty public IP set");
    let client = match Client::builder()
        .redirect(Policy::none())
        .timeout(Duration::from_secs(config.timeout_seconds as u64))
        .user_agent("Argus-Site-Monitor/0.1")
        .resolve(&host, SocketAddr::new(pinned_ip, port))
        .build()
    {
        Ok(client) => client,
        Err(error) => {
            result.error_code = Some("CLIENT_BUILD_FAILED".into());
            result.error_message = Some(truncate_error(&error.to_string()));
            return result;
        }
    };

    match request_status(&client, target.clone()).await {
        Ok((status, latency)) => {
            result.http_status = Some(status as i32);
            result.http_latency_ms = Some(latency);
            if scheme == "https" {
                result.tls_status = "VALID".into();
            }
            result.overall_status = if healthy_status(status) {
                "HEALTHY".into()
            } else {
                "DOWN".into()
            };
        }
        Err(error) => {
            result.overall_status = "DOWN".into();
            result.error_code = Some("HTTP_REQUEST_FAILED".into());
            result.error_message = Some(truncate_error(&error.to_string()));
            return result;
        }
    }

    if config.check_robots {
        match request_status(&client, origin_path(target, "/robots.txt")).await {
            Ok((status, _)) => {
                result.robots_status = Some(status as i32);
                if !healthy_status(status) && result.overall_status == "HEALTHY" {
                    result.overall_status = "DEGRADED".into();
                }
            }
            Err(error) => {
                mark_auxiliary_error(&mut result, "ROBOTS_CHECK_FAILED", &error.to_string())
            }
        }
    }
    if config.check_sitemap {
        match request_status(&client, origin_path(target, "/sitemap.xml")).await {
            Ok((status, _)) => {
                result.sitemap_status = Some(status as i32);
                if !healthy_status(status) && result.overall_status == "HEALTHY" {
                    result.overall_status = "DEGRADED".into();
                }
            }
            Err(error) => {
                mark_auxiliary_error(&mut result, "SITEMAP_CHECK_FAILED", &error.to_string())
            }
        }
    }
    result
}

async fn request_status(client: &Client, url: Url) -> Result<(u16, i64), reqwest::Error> {
    let started = Instant::now();
    let response = client.get(url).send().await?;
    let elapsed = started.elapsed().as_millis().min(i64::MAX as u128) as i64;
    Ok((response.status().as_u16(), elapsed))
}

fn origin_path(target: &Url, path: &str) -> Url {
    let mut url = target.clone();
    url.set_path(path);
    url.set_query(None);
    url.set_fragment(None);
    url
}

fn healthy_status(status: u16) -> bool {
    (200..400).contains(&status)
}

fn mark_auxiliary_error(result: &mut ProbeResult, code: &str, message: &str) {
    if result.overall_status == "HEALTHY" {
        result.overall_status = "DEGRADED".into();
    }
    if result.error_code.is_none() {
        result.error_code = Some(code.into());
        result.error_message = Some(truncate_error(message));
    }
}

fn normalize_target_url(value: &str) -> Result<Url, SiteMonitoringError> {
    let mut url = Url::parse(value.trim()).map_err(|_| SiteMonitoringError::Invalid)?;
    if !matches!(url.scheme(), "http" | "https")
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url
            .host_str()
            .is_some_and(|host| host.parse::<IpAddr>().is_ok())
    {
        return Err(SiteMonitoringError::Invalid);
    }
    let port = url
        .port_or_known_default()
        .ok_or(SiteMonitoringError::Invalid)?;
    if !matches!(port, 80 | 443) {
        return Err(SiteMonitoringError::Invalid);
    }
    url.set_fragment(None);
    Ok(url)
}

fn is_public_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => is_public_ipv4(ip),
        IpAddr::V6(ip) => is_public_ipv6(ip),
    }
}

fn is_public_ipv4(ip: Ipv4Addr) -> bool {
    let [a, b, c, d] = ip.octets();
    if a == 0
        || a == 10
        || a == 127
        || a >= 224
        || (a == 100 && (64..=127).contains(&b))
        || (a == 169 && b == 254)
        || (a == 172 && (16..=31).contains(&b))
        || (a == 192 && b == 168)
        || (a == 192 && b == 0 && c == 0)
        || (a == 192 && b == 0 && c == 2)
        || (a == 192 && b == 88 && c == 99)
        || (a == 198 && (b == 18 || b == 19))
        || (a == 198 && b == 51 && c == 100)
        || (a == 203 && b == 0 && c == 113)
        || (a == 255 && b == 255 && c == 255 && d == 255)
    {
        return false;
    }
    true
}

fn is_public_ipv6(ip: Ipv6Addr) -> bool {
    let segments = ip.segments();
    let first = segments[0];
    let global_unicast = (0x2000..=0x3fff).contains(&first);
    let documentation = segments[0] == 0x2001 && segments[1] == 0x0db8;
    global_unicast && !documentation
}

fn truncate_error(value: &str) -> String {
    value.chars().take(500).collect()
}

fn config_from_row(row: sqlx::postgres::PgRow) -> Result<SiteMonitorConfig, SiteMonitoringError> {
    Ok(SiteMonitorConfig {
        id: row.get("id"),
        site_id: row.get("site_id"),
        target_url: row.get("target_url"),
        check_robots: row.get("check_robots"),
        check_sitemap: row.get("check_sitemap"),
        timeout_seconds: row.get("timeout_seconds"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}

fn check_from_row(row: sqlx::postgres::PgRow) -> Result<SiteMonitorCheck, SiteMonitoringError> {
    let resolved: serde_json::Value = row.get("resolved_ips");
    let resolved_ips =
        serde_json::from_value(resolved).map_err(|_| SiteMonitoringError::Invalid)?;
    Ok(SiteMonitorCheck {
        id: row.get("id"),
        site_id: row.get("site_id"),
        config_id: row.get("config_id"),
        overall_status: row.get("overall_status"),
        target_url: row.get("target_url"),
        resolved_ips,
        dns_ok: row.get("dns_ok"),
        http_status: row.get("http_status"),
        http_latency_ms: row.get("http_latency_ms"),
        tls_status: row.get("tls_status"),
        robots_status: row.get("robots_status"),
        sitemap_status: row.get("sitemap_status"),
        error_code: row.get("error_code"),
        error_message: row.get("error_message"),
        checked_by: row.get("checked_by"),
        checked_at: row.get("checked_at"),
    })
}

fn default_true() -> bool {
    true
}
fn default_timeout() -> i32 {
    10
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

fn map_monitoring(error: SiteMonitoringError) -> ApiError {
    match error {
        SiteMonitoringError::NotFound => api_error(
            StatusCode::NOT_FOUND,
            "SITE_MONITOR_NOT_FOUND",
            "site or monitor configuration not found",
        ),
        SiteMonitoringError::Invalid => api_error(
            StatusCode::BAD_REQUEST,
            "INVALID_REQUEST",
            "invalid site monitoring request",
        ),
        other => {
            tracing::error!(error=%other, "site monitoring storage error");
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
    fn target_rejects_credentials_ips_and_custom_ports() {
        assert!(normalize_target_url("https://example.com/").is_ok());
        assert!(normalize_target_url("https://user:pass@example.com/").is_err());
        assert!(normalize_target_url("http://127.0.0.1/").is_err());
        assert!(normalize_target_url("https://example.com:8443/").is_err());
    }

    #[test]
    fn ipv4_private_special_and_documentation_are_blocked() {
        for address in [
            "10.0.0.1",
            "100.64.0.1",
            "127.0.0.1",
            "169.254.1.1",
            "172.16.0.1",
            "192.168.1.1",
            "192.0.2.1",
            "198.18.0.1",
            "198.51.100.1",
            "203.0.113.1",
        ] {
            assert!(!is_public_ip(address.parse().unwrap()), "{address}");
        }
        assert!(is_public_ip("1.1.1.1".parse().unwrap()));
    }

    #[test]
    fn ipv6_requires_global_unicast_and_blocks_documentation() {
        assert!(!is_public_ip("::1".parse().unwrap()));
        assert!(!is_public_ip("fc00::1".parse().unwrap()));
        assert!(!is_public_ip("fe80::1".parse().unwrap()));
        assert!(!is_public_ip("2001:db8::1".parse().unwrap()));
        assert!(is_public_ip("2606:4700:4700::1111".parse().unwrap()));
    }
}
