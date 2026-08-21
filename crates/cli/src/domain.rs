use crate::lifecycle::{self, DEFAULT_INSTALL_DIR};
use anyhow::{Context, Result, bail};
use std::{
    collections::BTreeMap,
    fs,
    net::{IpAddr, ToSocketAddrs},
    os::unix::fs::PermissionsExt,
    path::Path,
    process::Command,
    thread,
    time::Duration,
};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Domains {
    pub web: String,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DomainResolution {
    pub domain: String,
    pub addresses: Vec<IpAddr>,
}

pub fn validate_domain(value: &str) -> Result<()> {
    if !value.contains('.')
        || value.starts_with('.')
        || value.ends_with('.')
        || value
            .bytes()
            .any(|b| !(b.is_ascii_alphanumeric() || b == b'.' || b == b'-'))
    {
        bail!("invalid fully-qualified domain: {value}");
    }
    Ok(())
}

pub fn normalize_domains(web: &str, content: Option<&str>) -> Result<Domains> {
    let web = web.trim().to_ascii_lowercase();
    validate_domain(&web)?;
    let content = content
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_ascii_lowercase)
        .unwrap_or_else(|| format!("content.{web}"));
    validate_domain(&content)?;
    if web == content {
        bail!("Web and content domains must differ");
    }
    Ok(Domains { web, content })
}

pub fn resolve_domain(value: &str) -> Result<Vec<IpAddr>> {
    validate_domain(value)?;
    let mut addresses = (value, 443)
        .to_socket_addrs()
        .with_context(|| {
            format!(
                "DNS lookup failed for {value}; configure an A, AAAA, or CNAME record before continuing"
            )
        })?
        .map(|address| address.ip())
        .collect::<Vec<_>>();
    addresses.sort_unstable();
    addresses.dedup();
    if addresses.is_empty() {
        bail!(
            "DNS lookup returned no addresses for {value}; configure an A, AAAA, or CNAME record before continuing"
        );
    }
    Ok(addresses)
}

pub fn require_domain_resolution(value: &str) -> Result<()> {
    resolve_domain(value).map(|_| ())
}

pub fn caddy_tls_error(logs: &str, domains: &[&str]) -> Option<String> {
    let mut limited = BTreeMap::new();
    for line in logs
        .lines()
        .filter(|line| line.contains("rateLimited") || line.contains("too many certificates"))
    {
        for domain in domains {
            if !line.contains(domain) {
                continue;
            }
            let retry_after = line
                .split_once("retry after ")
                .map(|(_, remainder)| remainder)
                .and_then(|remainder| {
                    remainder
                        .split_once(": see ")
                        .map(|(value, _)| value)
                        .or_else(|| remainder.split_once(" (ca=").map(|(value, _)| value))
                })
                .map(|value| value.trim_matches(|character| character == '\"' || character == ']'))
                .unwrap_or("the time specified by Let's Encrypt");
            let retry_after = retry_after.to_string();
            limited
                .entry((*domain).to_string())
                .and_modify(|current| {
                    if retry_after > *current {
                        *current = retry_after.clone();
                    }
                })
                .or_insert(retry_after);
        }
    }

    if limited.is_empty() {
        return None;
    }

    let details = limited
        .iter()
        .map(|(domain, retry_after)| format!("{domain}: {retry_after}"))
        .collect::<Vec<_>>()
        .join(", ");
    Some(format!(
        "Let's Encrypt rate-limited TLS certificate issuance ({details}). Do not retry the installer or domain change before the latest listed time; repeated attempts cannot bypass the certificate authority limit"
    ))
}

pub fn resolve_domains(domains: &Domains) -> Result<Vec<DomainResolution>> {
    Ok(vec![
        DomainResolution {
            domain: domains.web.clone(),
            addresses: resolve_domain(&domains.web)?,
        },
        DomainResolution {
            domain: domains.content.clone(),
            addresses: resolve_domain(&domains.content)?,
        },
    ])
}

pub fn installed_domains() -> Result<Domains> {
    let install_dir = lifecycle::env_path("ARGUS_INSTALL_DIR", DEFAULT_INSTALL_DIR);
    installed_domains_at(&install_dir)
}

pub fn check_installed_domains() -> Result<Vec<DomainResolution>> {
    let domains = installed_domains()?;
    resolve_domains(&domains)
}

pub fn set_installed_domains(web: &str, content: Option<&str>) -> Result<Domains> {
    lifecycle::require_root().context("domain changes must run as root")?;
    if !lifecycle::command_exists("docker") {
        bail!("docker is required");
    }

    let domains = normalize_domains(web, content)?;

    // DNS is deliberately checked before touching the installation. This only requires
    // successful resolution, so Cloudflare-proxied records are valid and do not need to
    // resolve to the origin server's public address.
    resolve_domains(&domains)?;

    let install_dir = lifecycle::env_path("ARGUS_INSTALL_DIR", DEFAULT_INSTALL_DIR);
    ensure_no_remote_agents(&install_dir)?;
    apply_installed_domains(&install_dir, &domains)?;
    Ok(domains)
}

fn installed_domains_at(install_dir: &Path) -> Result<Domains> {
    let env_file = install_dir.join(".env");
    if !env_file.is_file() {
        bail!(
            "Argus control-plane environment not found at {}",
            env_file.display()
        );
    }
    let values = lifecycle::read_env_file(&env_file)?;
    let web = values
        .get("ARGUS_DOMAIN")
        .cloned()
        .context("ARGUS_DOMAIN is missing from the installed environment")?;
    let content = values
        .get("ARGUS_CONTENT_DOMAIN")
        .cloned()
        .context("ARGUS_CONTENT_DOMAIN is missing from the installed environment")?;
    normalize_domains(&web, Some(&content))
}

fn ensure_no_remote_agents(install_dir: &Path) -> Result<()> {
    let values = lifecycle::read_env_file(&install_dir.join(".env"))?;
    let local_server_id = values
        .get("ARGUS_SERVER_ID")
        .context("ARGUS_SERVER_ID is missing from the installed environment")?;
    let sql = format!(
        "SELECT COUNT(*) FROM agents WHERE server_id <> '{}'::uuid;",
        local_server_id.replace('\'', "''")
    );
    let output = compose_output(
        install_dir,
        &[
            "exec", "-T", "postgres", "psql", "-At", "-U", "argus", "-d", "argus", "-c", &sql,
        ],
    )
    .context("check managed agents before domain change")?;
    let remote_agents = output
        .trim()
        .parse::<u64>()
        .context("parse managed agent count")?;
    if remote_agents > 0 {
        bail!(
            "domain change would disconnect {remote_agents} managed agent(s); remote agent URL migration is not implemented yet"
        );
    }
    Ok(())
}

fn apply_installed_domains(install_dir: &Path, domains: &Domains) -> Result<()> {
    let env_file = install_dir.join(".env");
    let compose_file = install_dir.join("compose.yaml");
    let caddy_template = install_dir.join("Caddyfile.template");
    let caddy_file = install_dir.join("Caddyfile");
    for path in [&env_file, &compose_file, &caddy_template, &caddy_file] {
        if !path.is_file() {
            bail!("required installed file is missing: {}", path.display());
        }
    }

    let old_env = fs::read(&env_file).with_context(|| format!("read {}", env_file.display()))?;
    let old_caddy =
        fs::read(&caddy_file).with_context(|| format!("read {}", caddy_file.display()))?;
    let mut values = lifecycle::read_env_file(&env_file)?;
    values.insert("ARGUS_DOMAIN".to_string(), domains.web.clone());
    values.insert("ARGUS_CONTENT_DOMAIN".to_string(), domains.content.clone());

    let template = fs::read_to_string(&caddy_template)
        .with_context(|| format!("read {}", caddy_template.display()))?;
    let rendered = template
        .replace("__ARGUS_DOMAIN__", &domains.web)
        .replace("__ARGUS_CONTENT_DOMAIN__", &domains.content);
    if rendered.contains("__ARGUS_DOMAIN__") || rendered.contains("__ARGUS_CONTENT_DOMAIN__") {
        bail!("Caddy template still contains unresolved domain placeholders");
    }

    let temporary_caddy = install_dir.join(format!(
        ".Caddyfile.domain-{}-{}",
        std::process::id(),
        Uuid::new_v4().simple()
    ));
    fs::write(&temporary_caddy, rendered)
        .with_context(|| format!("write {}", temporary_caddy.display()))?;
    fs::set_permissions(&temporary_caddy, fs::Permissions::from_mode(0o640))?;

    if let Err(error) = validate_caddy(&temporary_caddy) {
        let _ = fs::remove_file(&temporary_caddy);
        return Err(error.context("new Caddy configuration is invalid"));
    }

    let apply_result = (|| -> Result<()> {
        write_env_map(&env_file, &values)?;
        fs::rename(&temporary_caddy, &caddy_file)
            .with_context(|| format!("replace {}", caddy_file.display()))?;
        fs::set_permissions(&caddy_file, fs::Permissions::from_mode(0o640))?;

        run_compose(install_dir, &["config"])
            .context("validate Compose configuration after domain change")?;
        recreate_domain_services(install_dir)
            .context("recreate domain-dependent Argus services")?;
        wait_for_domain_https(install_dir, domains).context("verify HTTPS after domain change")?;
        Ok(())
    })();

    if let Err(error) = apply_result {
        let logs =
            compose_output(install_dir, &["logs", "--tail=200", "caddy"]).unwrap_or_default();
        let error = match caddy_tls_error(&logs, &[&domains.web, &domains.content]) {
            Some(message) => error.context(message),
            None => error,
        };
        let rollback_result = rollback_domain_change(install_dir, &old_env, &old_caddy);
        return match rollback_result {
            Ok(()) => Err(error.context("domain change failed; previous domains were restored")),
            Err(rollback_error) => Err(error.context(format!(
                "domain change failed and rollback also failed: {rollback_error:#}"
            ))),
        };
    }

    Ok(())
}

fn validate_caddy(path: &Path) -> Result<()> {
    let mount = format!("{}:/etc/caddy/Caddyfile:ro", path.display());
    lifecycle::run_quiet(
        "docker",
        &[
            "run",
            "--rm",
            "-v",
            &mount,
            "caddy:2-alpine",
            "caddy",
            "validate",
            "--config",
            "/etc/caddy/Caddyfile",
        ],
    )
}

fn write_env_map(path: &Path, values: &BTreeMap<String, String>) -> Result<()> {
    let pairs = values
        .iter()
        .map(|(key, value)| (key.as_str(), value.as_str()))
        .collect::<Vec<_>>();
    lifecycle::write_env_file(path, &pairs, 0o600)
}

fn compose_args(install_dir: &Path, args: &[&str]) -> Vec<String> {
    let mut result = vec![
        "compose".to_string(),
        "--project-directory".to_string(),
        install_dir.display().to_string(),
        "--env-file".to_string(),
        install_dir.join(".env").display().to_string(),
        "-f".to_string(),
        install_dir.join("compose.yaml").display().to_string(),
    ];
    result.extend(args.iter().map(|value| (*value).to_string()));
    result
}

fn run_compose(install_dir: &Path, args: &[&str]) -> Result<()> {
    let args = compose_args(install_dir, args);
    let refs = args.iter().map(String::as_str).collect::<Vec<_>>();
    lifecycle::run_quiet("docker", &refs)
}

fn compose_output(install_dir: &Path, args: &[&str]) -> Result<String> {
    let args = compose_args(install_dir, args);
    let refs = args.iter().map(String::as_str).collect::<Vec<_>>();
    lifecycle::output("docker", &refs)
}

fn recreate_domain_services(install_dir: &Path) -> Result<()> {
    // Caddy is recreated as well as the app services. Its Caddyfile is a bind mount,
    // so recreating ensures Docker binds the replacement file rather than retaining the
    // inode that was mounted before the atomic configuration swap.
    run_compose(
        install_dir,
        &[
            "up",
            "-d",
            "--force-recreate",
            "--wait",
            "--wait-timeout",
            "120",
            "web",
            "content",
            "caddy",
        ],
    )
}

fn https_health_reachable(domain: &str) -> bool {
    Command::new("curl")
        .args([
            "-sS",
            "--connect-timeout",
            "5",
            "-o",
            "/dev/null",
            "-w",
            "%{http_code}",
            &format!("https://{domain}/healthz"),
        ])
        .output()
        .is_ok_and(|output| output.status.success() && output.stdout == b"200")
}

fn wait_for_domain_https(install_dir: &Path, domains: &Domains) -> Result<()> {
    for _ in 0..60 {
        if https_health_reachable(&domains.web) && https_health_reachable(&domains.content) {
            return Ok(());
        }

        let logs =
            compose_output(install_dir, &["logs", "--tail=200", "caddy"]).unwrap_or_default();
        if let Some(error) = caddy_tls_error(&logs, &[&domains.web, &domains.content]) {
            bail!(error);
        }
        thread::sleep(Duration::from_secs(2));
    }

    bail!(
        "HTTPS health checks did not become reachable for {} and {}; verify DNS and external access to ports 80/443",
        domains.web,
        domains.content
    )
}

fn rollback_domain_change(install_dir: &Path, env: &[u8], caddy: &[u8]) -> Result<()> {
    let env_file = install_dir.join(".env");
    let caddy_file = install_dir.join("Caddyfile");
    fs::write(&env_file, env).with_context(|| format!("restore {}", env_file.display()))?;
    fs::set_permissions(&env_file, fs::Permissions::from_mode(0o600))?;
    fs::write(&caddy_file, caddy).with_context(|| format!("restore {}", caddy_file.display()))?;
    fs::set_permissions(&caddy_file, fs::Permissions::from_mode(0o640))?;
    recreate_domain_services(install_dir).context("restore domain-dependent services")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn domain_validation_accepts_normal_fqdns() {
        for domain in [
            "argus.example.com",
            "content.argus.example.com",
            "a-b.example.co.uk",
        ] {
            validate_domain(domain).unwrap();
        }
    }

    #[test]
    fn domain_validation_rejects_invalid_names() {
        for domain in [
            "localhost",
            ".example.com",
            "example.com.",
            "https://example.com",
            "example com",
        ] {
            assert!(validate_domain(domain).is_err(), "{domain}");
        }
    }

    #[test]
    fn domain_set_normalizes_and_derives_content_domain() {
        assert_eq!(
            normalize_domains(" ARGUS.EXAMPLE.COM ", None).unwrap(),
            Domains {
                web: "argus.example.com".to_string(),
                content: "content.argus.example.com".to_string(),
            }
        );
        assert_eq!(
            normalize_domains("argus.example.com", Some("CMS.EXAMPLE.COM")).unwrap(),
            Domains {
                web: "argus.example.com".to_string(),
                content: "cms.example.com".to_string(),
            }
        );
    }

    #[test]
    fn domain_set_rejects_equal_web_and_content_domains() {
        assert!(normalize_domains("argus.example.com", Some("ARGUS.EXAMPLE.COM")).is_err());
    }

    #[test]
    fn compose_args_are_bound_to_installed_files() {
        let install_dir = PathBuf::from("/srv/argus");
        assert_eq!(
            compose_args(&install_dir, &["config"]),
            vec![
                "compose",
                "--project-directory",
                "/srv/argus",
                "--env-file",
                "/srv/argus/.env",
                "-f",
                "/srv/argus/compose.yaml",
                "config",
            ]
        );
    }

    #[test]
    fn caddy_rate_limit_error_reports_domains_and_retry_times() {
        let logs = r#"
{"identifier":"content.example.com","error":"HTTP 429 rateLimited - too many certificates; retry after 2026-08-22 03:15:46 UTC: see https://letsencrypt.org/docs/rate-limits/"}
{"identifier":"app.example.com","error":"HTTP 429 rateLimited - too many certificates; retry after 2026-08-21 22:04:12 UTC: see https://letsencrypt.org/docs/rate-limits/"}
"#;
        let error = caddy_tls_error(logs, &["app.example.com", "content.example.com"]).unwrap();
        assert!(error.contains("app.example.com: 2026-08-21 22:04:12 UTC"));
        assert!(error.contains("content.example.com: 2026-08-22 03:15:46 UTC"));
        assert!(error.contains("Do not retry"));
    }

    #[test]
    fn caddy_tls_error_ignores_unrelated_failures() {
        assert!(caddy_tls_error("connection refused", &["app.example.com"]).is_none());
    }
}
