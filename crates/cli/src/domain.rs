use crate::lifecycle::{self, DEFAULT_INSTALL_DIR};
use anyhow::{Context, Result, bail};
use std::{
    collections::BTreeMap,
    fs,
    net::{IpAddr, ToSocketAddrs},
    os::unix::fs::PermissionsExt,
    path::Path,
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
        Ok(())
    })();

    if let Err(error) = apply_result {
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

fn rollback_domain_change(install_dir: &Path, env: &[u8], caddy: &[u8]) -> Result<()> {
    let env_file = install_dir.join(".env");
    let caddy_file = install_dir.join("Caddyfile");
    fs::write(&env_file, env).with_context(|| format!("restore {}", env_file.display()))?;
    fs::set_permissions(&env_file, fs::Permissions::from_mode(0o600))?;
    fs::write(&caddy_file, caddy)
        .with_context(|| format!("restore {}", caddy_file.display()))?;
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
}
