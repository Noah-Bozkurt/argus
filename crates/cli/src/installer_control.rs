use super::installer_shared::{ControlConfig, Installer, TlsMode};
use anyhow::{Context, Result, bail};
use cli::{
    domain,
    lifecycle::{self, output, write_env_file},
};
use secrecy::ExposeSecret;
use serde_json::{Value, json};
use std::{
    env, fs,
    os::unix::fs::PermissionsExt,
    process::{Command, Stdio},
    thread,
    time::Duration,
};

fn public_acme_options(email: &str, zerossl_first: bool) -> String {
    let lets_encrypt = "\tcert_issuer acme https://acme-v02.api.letsencrypt.org/directory";
    let zerossl = "\tcert_issuer acme https://acme.zerossl.com/v2/DV90";
    let issuers = if zerossl_first {
        format!("{zerossl}\n{lets_encrypt}")
    } else {
        format!("{lets_encrypt}\n{zerossl}")
    };
    format!("{{\n\temail {email}\n{issuers}\n}}")
}

impl Installer {
    pub(crate) fn ensure_argus_user(&self) -> Result<()> {
        if !Command::new("getent")
            .args(["group", "argus"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|status| status.success())
        {
            lifecycle::run_quiet("groupadd", &["--system", "argus"])?;
        }
        if !Command::new("id")
            .arg("argus")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|status| status.success())
        {
            lifecycle::run_quiet(
                "useradd",
                &[
                    "--system",
                    "--gid",
                    "argus",
                    "--home-dir",
                    self.state_dir.to_str().context("state path")?,
                    "--shell",
                    "/usr/sbin/nologin",
                    "argus",
                ],
            )?;
        }
        fs::create_dir_all(&self.config_dir)?;
        fs::create_dir_all(self.config_dir.join("tls"))?;
        fs::create_dir_all(self.state_dir.join("backups"))?;
        lifecycle::run_quiet(
            "chown",
            &[
                "root:argus",
                self.config_dir.to_str().context("config path")?,
            ],
        )?;
        fs::set_permissions(&self.config_dir, fs::Permissions::from_mode(0o750))?;
        lifecycle::run_quiet(
            "chown",
            &[
                "-R",
                "argus:argus",
                self.state_dir.to_str().context("state path")?,
            ],
        )?;
        fs::set_permissions(&self.state_dir, fs::Permissions::from_mode(0o750))?;
        Ok(())
    }

    pub(crate) fn write_runtime_env(&self, config: &ControlConfig) -> Result<()> {
        let values = [
            ("ARGUS_REGISTRY", config.registry.as_str()),
            ("ARGUS_VERSION", config.version.as_str()),
            ("ARGUS_DOMAIN", config.domain.as_str()),
            ("ARGUS_CONTENT_DOMAIN", config.content_domain.as_str()),
            ("ARGUS_BASIC_AUTH_USER", config.basic_auth_user.as_str()),
            (
                "ARGUS_BASIC_AUTH_PASSWORD",
                config.basic_auth_password.expose_secret(),
            ),
            (
                "ARGUS_POSTGRES_PASSWORD",
                config.postgres_password.expose_secret(),
            ),
            ("ARGUS_WEB_API_TOKEN", config.web_api_token.expose_secret()),
            ("ARGUS_WORKER_TOKEN", config.worker_token.expose_secret()),
            (
                "ARGUS_CONTENT_SYNC_TOKEN",
                config.content_sync_token.expose_secret(),
            ),
            ("PAYLOAD_SECRET", config.payload_secret.expose_secret()),
            ("ARGUS_ORG_ID", config.org_id.as_str()),
            ("ARGUS_USER_ID", config.user_id.as_str()),
            (
                "ARGUS_BOOTSTRAP_PROJECT_ID",
                config.bootstrap_project_id.as_str(),
            ),
            (
                "ARGUS_BOOTSTRAP_ENVIRONMENT_ID",
                config.bootstrap_environment_id.as_str(),
            ),
            ("ARGUS_SERVER_ID", config.server_id.as_str()),
            ("ARGUS_GITHUB_TOKEN", config.github_token.expose_secret()),
            ("ARGUS_RUST_LOG", config.rust_log.as_str()),
            ("ARGUS_ACME_EMAIL", config.acme_email.as_str()),
            (
                "ARGUS_TLS_MODE",
                match config.tls_mode {
                    TlsMode::PublicAcme => "public-acme",
                    TlsMode::CloudflareOrigin => "cloudflare-origin",
                },
            ),
            (
                "ARGUS_CONFIG_DIR",
                self.config_dir.to_str().context("config path")?,
            ),
        ];
        write_env_file(&self.env_file(), &values, 0o600)
    }

    pub(crate) fn generate_caddy_config(&self, config: &ControlConfig) -> Result<()> {
        self.generate_caddy_config_with(config, false, false)
    }

    pub(crate) fn regenerate_caddy_config(&self, config: &ControlConfig) -> Result<()> {
        self.generate_caddy_config_with(config, true, false)
    }

    fn generate_caddy_config_with(
        &self,
        config: &ControlConfig,
        force: bool,
        zerossl_first: bool,
    ) -> Result<()> {
        if !force
            && self.caddy_file().is_file()
            && env::var("ARGUS_RECONFIGURE_CADDY").as_deref() != Ok("1")
        {
            self.ui.detail("Preserving existing Caddyfile");
            return Ok(());
        }
        let hash = self.docker_output(&[
            "run",
            "--rm",
            "caddy:2-alpine",
            "caddy",
            "hash-password",
            "--plaintext",
            config.basic_auth_password.expose_secret(),
        ])?;
        let template = fs::read_to_string(self.install_dir.join("Caddyfile.template"))?;
        let (global_options, tls) = match config.tls_mode {
            TlsMode::PublicAcme => (
                public_acme_options(&config.acme_email, zerossl_first),
                String::new(),
            ),
            TlsMode::CloudflareOrigin => (
                String::new(),
                "\ttls /etc/caddy/argus-tls/origin.crt /etc/caddy/argus-tls/origin.key\n"
                    .to_string(),
            ),
        };
        let rendered = template
            .replace("__ARGUS_GLOBAL_OPTIONS__", &global_options)
            .replace("__ARGUS_TLS__", &tls)
            .replace("__ARGUS_DOMAIN__", &config.domain)
            .replace("__ARGUS_CONTENT_DOMAIN__", &config.content_domain)
            .replace("__BASIC_AUTH_USER__", &config.basic_auth_user)
            .replace("__BASIC_AUTH_HASH__", &hash);
        if rendered.contains("__ARGUS_") || rendered.contains("__BASIC_AUTH_") {
            bail!("Caddy template still contains unresolved placeholders");
        }
        fs::write(self.caddy_file(), rendered)?;
        fs::set_permissions(self.caddy_file(), fs::Permissions::from_mode(0o640))?;
        self.docker_status(&[
            "run",
            "--rm",
            "-v",
            &format!("{}:/etc/caddy/Caddyfile:ro", self.caddy_file().display()),
            "-v",
            &format!(
                "{}:/etc/caddy/argus-tls:ro",
                self.config_dir.join("tls").display()
            ),
            "caddy:2-alpine",
            "caddy",
            "validate",
            "--config",
            "/etc/caddy/Caddyfile",
        ])
    }

    fn retry_with_zerossl(&self, config: &ControlConfig) -> Result<()> {
        self.ui
            .warning("Let's Encrypt is rate-limited; retrying certificate issuance with ZeroSSL.");
        self.generate_caddy_config_with(config, true, true)?;
        self.compose_status(&["up", "-d", "--force-recreate", "caddy"])?;

        let web = format!("https://{}/healthz", config.domain);
        let content = format!("https://{}/healthz", config.content_domain);
        let succeeded = self.wait_for_https(&web) && self.wait_for_https(&content);
        let logs = self
            .compose_output(&["logs", "--tail=300", "caddy"])
            .unwrap_or_default();

        self.generate_caddy_config_with(config, true, false)?;
        self.compose_status(&[
            "exec",
            "-T",
            "caddy",
            "caddy",
            "reload",
            "--config",
            "/etc/caddy/Caddyfile",
        ])?;

        if !succeeded {
            bail!(
                "ZeroSSL fallback did not make HTTPS reachable. Caddy logs:\n{}",
                domain::redact_caddy_tls_logs(&logs, &[&config.domain, &config.content_domain])
            );
        }
        Ok(())
    }

    fn configure_firewall_if_active(&self) -> Result<()> {
        if !lifecycle::command_exists("ufw") {
            return Ok(());
        }
        let status = output("ufw", &["status"])?;
        if status.lines().any(|line| line.trim() == "Status: active") {
            for rule in ["80/tcp", "443/tcp", "443/udp"] {
                lifecycle::run_quiet("ufw", &["allow", rule])?;
            }
        }
        Ok(())
    }

    pub(crate) fn pull_control_plane_images(&self) -> Result<()> {
        self.compose_status(&["config", "--quiet"])?;
        let images = self.compose_output(&["config", "--images"])?;
        let images = images
            .lines()
            .map(str::trim)
            .filter(|image| !image.is_empty())
            .map(ToOwned::to_owned)
            .collect::<Vec<_>>();
        if images.is_empty() {
            bail!("Compose configuration did not resolve any images");
        }
        self.ui.pull_images(&images)
    }

    pub(crate) fn start_control_plane(&self) -> Result<()> {
        self.configure_firewall_if_active()?;
        self.compose_status(&["up", "-d"])?;
        for _ in 0..90 {
            if Command::new("curl")
                .args(["-fsS", "http://127.0.0.1:8080/health"])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .is_ok_and(|status| status.success())
            {
                return Ok(());
            }
            thread::sleep(Duration::from_secs(2));
        }
        let _ = self.compose_status(&["ps"]);
        let _ = self.compose_status(&["logs", "--tail=120", "control-api", "postgres"]);
        bail!("Control API did not become healthy")
    }

    pub(crate) fn bootstrap_control_plane(&self, config: &ControlConfig) -> Result<()> {
        let hostname = output("hostname", &["-f"]).or_else(|_| output("hostname", &[]))?;
        let sql = r#"
INSERT INTO organizations(id,name) VALUES (:'org_id'::uuid, :'org_name') ON CONFLICT(id) DO NOTHING;
INSERT INTO users(id,organization_id,email) VALUES (:'user_id'::uuid, :'org_id'::uuid, :'operator_email') ON CONFLICT(id) DO NOTHING;
INSERT INTO projects(id,organization_id,name,client_id,description,preset,status,tags)
VALUES (:'project_id'::uuid, :'org_id'::uuid, 'Argus Control Plane', NULL, 'Bootstrap project for the server running Argus itself.', 'infrastructure', 'ACTIVE', '[]'::jsonb) ON CONFLICT(id) DO NOTHING;
INSERT INTO environments(id,organization_id,project_id,name,type,description,is_protected,sort_order)
VALUES (:'environment_id'::uuid, :'org_id'::uuid, :'project_id'::uuid, 'Control Plane', 'production', 'Environment containing the Argus host.', TRUE, 0) ON CONFLICT(id) DO NOTHING;
INSERT INTO servers(id,organization_id,project_id,environment_id,hostname)
VALUES (:'server_id'::uuid, :'org_id'::uuid, :'project_id'::uuid, :'environment_id'::uuid, :'host_name') ON CONFLICT(id) DO NOTHING;
"#;
        self.compose_with_input(
            &[
                "exec",
                "-T",
                "postgres",
                "psql",
                "-v",
                "ON_ERROR_STOP=1",
                "-U",
                "argus",
                "-d",
                "argus",
                "-v",
                &format!("org_id={}", config.org_id),
                "-v",
                &format!("user_id={}", config.user_id),
                "-v",
                &format!("project_id={}", config.bootstrap_project_id),
                "-v",
                &format!("environment_id={}", config.bootstrap_environment_id),
                "-v",
                &format!("server_id={}", config.server_id),
                "-v",
                &format!("org_name={}", config.org_name),
                "-v",
                &format!("operator_email={}", config.operator_email),
                "-v",
                &format!("host_name={hostname}"),
            ],
            sql.as_bytes(),
        )
    }

    pub(crate) fn write_helper_env(&self) -> Result<()> {
        let allowed = env::var("ARGUS_ALLOWED_SERVICES").unwrap_or_default();
        let backup_dir = self.state_dir.join("backups").display().to_string();
        write_env_file(
            &self.config_dir.join("helper.env"),
            &[
                ("ARGUS_HELPER_SOCKET", "/run/argus/helper.sock"),
                ("ARGUS_ALLOWED_SERVICES", allowed.as_str()),
                ("ARGUS_BACKUP_DIR", backup_dir.as_str()),
            ],
            0o640,
        )?;
        lifecycle::run_quiet(
            "chown",
            &[
                "root:argus",
                self.config_dir
                    .join("helper.env")
                    .to_str()
                    .context("helper env path")?,
            ],
        )
    }

    pub(crate) fn write_agent_env(
        &self,
        control_plane_url: &str,
        server_id: &str,
        rust_log: &str,
        enrollment_token: Option<&str>,
    ) -> Result<()> {
        let agent_config = self.state_dir.join("agent.json").display().to_string();
        let managed = env::var("ARGUS_MANAGED_SERVICES").unwrap_or_default();
        let mut values = vec![
            ("ARGUS_CONTROL_PLANE_URL", control_plane_url),
            ("ARGUS_SERVER_ID", server_id),
            ("ARGUS_AGENT_CONFIG", agent_config.as_str()),
            ("ARGUS_HELPER_SOCKET", "/run/argus/helper.sock"),
            ("ARGUS_MANAGED_SERVICES", managed.as_str()),
            ("RUST_LOG", rust_log),
        ];
        if let Some(token) = enrollment_token {
            values.push(("ARGUS_ENROLLMENT_TOKEN", token));
        }
        write_env_file(&self.config_dir.join("agent.env"), &values, 0o640)?;
        lifecycle::run_quiet(
            "chown",
            &[
                "root:argus",
                self.config_dir
                    .join("agent.env")
                    .to_str()
                    .context("agent env path")?,
            ],
        )
    }

    pub(crate) fn enroll_local_agent(&self, config: &ControlConfig) -> Result<()> {
        self.write_helper_env()?;
        lifecycle::run_quiet("systemctl", &["enable", "--now", "argus-helper.service"])?;
        let agent_json = self.state_dir.join("agent.json");
        if agent_json.is_file() && fs::metadata(&agent_json)?.len() > 0 {
            self.ui
                .detail("Existing local Agent identity found; skipping enrollment");
            self.write_agent_env(
                "http://127.0.0.1:8080",
                &config.server_id,
                &config.rust_log,
                None,
            )?;
            lifecycle::run_quiet("systemctl", &["enable", "--now", "argus-agent.service"])?;
            return Ok(());
        }
        let payload = json!({"server_id": config.server_id, "ttl_seconds": 1800}).to_string();
        let authorization = format!(
            "Authorization: Bearer {}",
            config.web_api_token.expose_secret()
        );
        let response = output(
            "curl",
            &[
                "-fsS",
                "-H",
                &authorization,
                "-H",
                &format!("x-argus-org-id: {}", config.org_id),
                "-H",
                &format!("x-argus-user-id: {}", config.user_id),
                "-H",
                "content-type: application/json",
                "-d",
                &payload,
                "http://127.0.0.1:8080/enrollment/tokens",
            ],
        )?;
        let token = serde_json::from_str::<Value>(&response)?
            .get("token")
            .and_then(Value::as_str)
            .context("enrollment response is missing token")?
            .to_string();
        self.write_agent_env(
            "http://127.0.0.1:8080",
            &config.server_id,
            &config.rust_log,
            Some(&token),
        )?;
        lifecycle::run_quiet("systemctl", &["enable", "--now", "argus-agent.service"])?;
        for _ in 0..60 {
            if agent_json.is_file() && fs::metadata(&agent_json)?.len() > 0 {
                let query = format!(
                    "SELECT EXISTS(SELECT 1 FROM agents WHERE server_id='{}'::uuid)",
                    config.server_id
                );
                if self.compose_output(&[
                    "exec", "-T", "postgres", "psql", "-U", "argus", "-d", "argus", "-Atc", &query,
                ])? == "t"
                {
                    self.write_agent_env(
                        "http://127.0.0.1:8080",
                        &config.server_id,
                        &config.rust_log,
                        None,
                    )?;
                    return Ok(());
                }
            }
            thread::sleep(Duration::from_secs(2));
        }
        let _ = lifecycle::run(
            "journalctl",
            &["-u", "argus-helper.service", "-n", "80", "--no-pager"],
        );
        let _ = lifecycle::run(
            "journalctl",
            &["-u", "argus-agent.service", "-n", "80", "--no-pager"],
        );
        bail!("local Argus Agent did not enroll successfully")
    }

    fn verify_compose_service(&self, service: &str, require_health: bool) -> Result<()> {
        let cid = self.compose_output(&["ps", "-q", service])?;
        if cid.is_empty() {
            bail!("Compose service '{service}' has no container");
        }
        if self.docker_output(&["inspect", "-f", "{{.State.Running}}", &cid])? != "true" {
            bail!("Compose service '{service}' is not running");
        }
        if require_health {
            let health = self.docker_output(&[
                "inspect",
                "-f",
                "{{if .State.Health}}{{.State.Health.Status}}{{else}}missing{{end}}",
                &cid,
            ])?;
            if health != "healthy" {
                bail!("Compose service '{service}' is not healthy (status: {health})");
            }
        }
        Ok(())
    }

    fn wait_for_https(&self, url: &str) -> bool {
        for _ in 0..45 {
            if let Ok(output) = Command::new("curl")
                .args([
                    "-sS",
                    "--connect-timeout",
                    "5",
                    "-o",
                    "/dev/null",
                    "-w",
                    "%{http_code}",
                    url,
                ])
                .output()
            {
                if matches!(
                    String::from_utf8_lossy(&output.stdout).trim(),
                    "200" | "401" | "302" | "307" | "308"
                ) {
                    return true;
                }
            }
            thread::sleep(Duration::from_secs(2));
        }
        false
    }

    pub(crate) fn verify_installation(&self, config: &ControlConfig) -> Result<()> {
        for service in ["postgres", "control-api", "worker", "web", "content"] {
            self.verify_compose_service(service, true)?;
        }
        self.verify_compose_service("caddy", false)?;
        lifecycle::run_quiet(
            "systemctl",
            &["is-active", "--quiet", "argus-helper.service"],
        )?;
        lifecycle::run_quiet(
            "systemctl",
            &["is-active", "--quiet", "argus-agent.service"],
        )?;
        lifecycle::run_quiet("curl", &["-fsS", "http://127.0.0.1:8080/health"])?;
        if !self.wait_for_https(&format!("https://{}/healthz", config.domain)) {
            let logs = self
                .compose_output(&["logs", "--tail=200", "caddy"])
                .unwrap_or_default();
            if let Some(error) = domain::caddy_tls_error(
                &logs,
                &[config.domain.as_str(), config.content_domain.as_str()],
            ) {
                if matches!(config.tls_mode, TlsMode::PublicAcme) {
                    self.ui.detail(&error);
                    self.retry_with_zerossl(config)?;
                } else {
                    bail!(error);
                }
            } else {
                let _ = self.compose_status(&["logs", "--tail=120", "caddy"]);
                bail!(
                    "Argus HTTPS did not become reachable. Verify DNS for {} and external firewall access to ports 80/443",
                    config.domain
                );
            }
        }
        if !self.wait_for_https(&format!("https://{}/healthz", config.content_domain)) {
            let logs = self
                .compose_output(&["logs", "--tail=200", "caddy"])
                .unwrap_or_default();
            if let Some(error) = domain::caddy_tls_error(
                &logs,
                &[config.domain.as_str(), config.content_domain.as_str()],
            ) {
                bail!(error);
            }
            let _ = self.compose_status(&["logs", "--tail=120", "caddy"]);
            bail!(
                "Payload HTTPS did not become reachable. Verify DNS for {} and external firewall access to ports 80/443",
                config.content_domain
            );
        }
        let basic_auth = format!(
            "{}:{}",
            config.basic_auth_user,
            config.basic_auth_password.expose_secret()
        );
        lifecycle::run_quiet(
            "curl",
            &[
                "-fsS",
                "-u",
                &basic_auth,
                &format!("https://{}/healthz", config.domain),
            ],
        )?;
        lifecycle::run_quiet(
            "curl",
            &[
                "-fsS",
                "-u",
                &basic_auth,
                &format!("https://{}/healthz", config.content_domain),
            ],
        )?;
        Ok(())
    }

    pub(crate) fn print_summary(&self, config: &ControlConfig) {
        println!();
        self.ui.success_title("Argus is ready");
        println!();
        println!("Web:      https://{}", config.domain);
        println!("Content:  https://{}", config.content_domain);
        println!("User:     {}", config.basic_auth_user);
        println!(
            "Password: {}",
            if config.generated_basic_password {
                "generated and stored securely"
            } else {
                "the password you entered"
            }
        );
        if config.generated_basic_password {
            println!();
            self.ui.detail(&format!(
                "A password was generated and stored in {}",
                self.env_file().display()
            ));
        }
        println!("Version:  {}", config.version);
        println!("Server:   {}", config.server_id);
        println!();
        self.ui
            .detail("Diagnostics: argusctl status / sudo argusctl smoke");
        if config.generated_basic_password {
            self.ui
                .detail("Show generated login: sudo argusctl credentials");
        }
        self.ui
            .detail("Update: sudo argusctl update --version main");
    }
}

#[cfg(test)]
mod tests {
    use super::public_acme_options;

    #[test]
    fn public_acme_issuer_order_can_be_reversed_for_fallback() {
        let normal = public_acme_options("admin@example.com", false);
        let fallback = public_acme_options("admin@example.com", true);
        assert!(normal.find("letsencrypt.org").unwrap() < normal.find("zerossl.com").unwrap());
        assert!(fallback.find("zerossl.com").unwrap() < fallback.find("letsencrypt.org").unwrap());
    }
}
