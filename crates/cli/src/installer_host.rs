use super::installer_shared::*;
use anyhow::{Context, Result, bail};
use cli::lifecycle::{
    self, RegistryCredentials, copy_file, output, prompt_line, prompt_secret, read_env_file,
    temp_dir,
};
use std::{
    collections::BTreeMap,
    env, fs,
    io::Write,
    os::unix::fs::PermissionsExt,
    path::Path,
    process::{Command, Stdio},
};

impl Installer {
    pub(crate) fn preflight(&self) -> Result<()> {
        let os = self.ui.working("Validating operating system, architecture and ports", || {
            lifecycle::require_root().context("installer must run as root")?;
            let os = parse_os_release()?;
            let id = os.get("ID").map(String::as_str).unwrap_or("");
            if id != "ubuntu" && id != "debian" { bail!("Argus currently supports Ubuntu or Debian only"); }
            let architecture = output("dpkg", &["--print-architecture"])?;
            if architecture != "amd64" { bail!("Argus currently supports amd64 only"); }
            if self.mode == InstallMode::ControlPlane && !self.compose_file().exists() {
                let sockets = output("ss", &["-ltnH"])?;
                for port in [80, 443] {
                    if sockets.lines().any(|line| line.split_whitespace().nth(3).is_some_and(|local| local.ends_with(&format!(":{port}")))) {
                        bail!("TCP port {port} is already in use; free the port before installing Argus");
                    }
                }
            }
            Ok(os)
        })?;
        self.ui
            .working("Refreshing package repositories", || self.apt(&["update"]))?;
        self.ui.working("Installing required host packages", || {
            self.apt(&[
                "install",
                "-y",
                "--no-install-recommends",
                "ca-certificates",
                "curl",
                "jq",
                "openssl",
                "iproute2",
                "ufw",
                "unattended-upgrades",
            ])
        })?;
        self.ui
            .working("Installing or validating Docker Engine", || {
                self.ensure_docker(&os)
            })?;
        Ok(())
    }

    fn apt(&self, args: &[&str]) -> Result<()> {
        let mut command = Command::new("apt-get");
        command.args(args).env("DEBIAN_FRONTEND", "noninteractive");
        self.finish_command(command, "apt-get")
    }

    pub(crate) fn finish_command(&self, mut command: Command, label: &str) -> Result<()> {
        command.stdout(Stdio::piped()).stderr(Stdio::piped());
        let output = command.output().with_context(|| format!("run {label}"))?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        if !stdout.is_empty() {
            self.ui.record(&format!("[{label} stdout]\n{stdout}"));
        }
        if !stderr.is_empty() {
            self.ui.record(&format!("[{label} stderr]\n{stderr}"));
        }
        if !output.status.success() {
            let stderr = stderr.trim().to_string();
            if stderr.is_empty() {
                bail!("{label} failed with {}", output.status);
            }
            bail!("{label} failed: {stderr}");
        }
        if self.ui.verbose {
            if !stdout.is_empty() {
                print!("{stdout}");
            }
            if !stderr.is_empty() {
                eprint!("{stderr}");
            }
        }
        Ok(())
    }

    fn ensure_docker(&self, os: &BTreeMap<String, String>) -> Result<()> {
        if lifecycle::command_exists("docker") {
            self.docker_status(&["compose", "version"])
                .context("Docker is installed but the Compose plugin is missing")?;
            lifecycle::run_quiet("systemctl", &["enable", "--now", "docker"])?;
            return Ok(());
        }
        for conflict in [
            "docker.io",
            "docker-compose",
            "docker-compose-v2",
            "podman-docker",
            "containerd",
            "runc",
        ] {
            let installed = Command::new("dpkg-query")
                .args(["-W", "-f=${Status}", conflict])
                .output()
                .ok()
                .filter(|out| out.status.success())
                .is_some_and(|out| {
                    String::from_utf8_lossy(&out.stdout).contains("install ok installed")
                });
            if installed {
                bail!(
                    "conflicting package '{conflict}' is installed; Argus will not remove an existing container stack automatically"
                );
            }
        }
        fs::create_dir_all("/etc/apt/keyrings")?;
        fs::set_permissions("/etc/apt/keyrings", fs::Permissions::from_mode(0o755))?;
        lifecycle::run_quiet(
            "curl",
            &[
                "-fsSL",
                &format!(
                    "https://download.docker.com/linux/{}/gpg",
                    os.get("ID").map(String::as_str).unwrap_or("ubuntu")
                ),
                "-o",
                "/etc/apt/keyrings/docker.asc",
            ],
        )?;
        fs::set_permissions(
            "/etc/apt/keyrings/docker.asc",
            fs::Permissions::from_mode(0o644),
        )?;
        let codename = os
            .get("UBUNTU_CODENAME")
            .or_else(|| os.get("VERSION_CODENAME"))
            .context("could not determine distribution codename")?;
        let architecture = output("dpkg", &["--print-architecture"])?;
        fs::write(
            "/etc/apt/sources.list.d/docker.sources",
            format!(
                "Types: deb\nURIs: https://download.docker.com/linux/{}\nSuites: {}\nComponents: stable\nArchitectures: {}\nSigned-By: /etc/apt/keyrings/docker.asc\n",
                os.get("ID").map(String::as_str).unwrap_or("ubuntu"),
                codename,
                architecture
            ),
        )?;
        self.apt(&["update"])?;
        self.apt(&[
            "install",
            "-y",
            "docker-ce",
            "docker-ce-cli",
            "containerd.io",
            "docker-buildx-plugin",
            "docker-compose-plugin",
        ])?;
        lifecycle::run_quiet("systemctl", &["enable", "--now", "docker"])?;
        self.docker_status(&["compose", "version"])
    }

    pub(crate) fn docker_command(&self, args: &[&str]) -> Command {
        let mut command = Command::new("docker");
        command.args(args).env("DOCKER_CONFIG", &self.docker_config);
        command
    }

    pub(crate) fn docker_status(&self, args: &[&str]) -> Result<()> {
        let command = self.docker_command(args);
        self.finish_command(command, "docker")
    }

    pub(crate) fn docker_output(&self, args: &[&str]) -> Result<String> {
        let output = self.docker_command(args).output().context("run docker")?;
        if !output.status.success() {
            bail!(
                "docker failed: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            );
        }
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    pub(crate) fn compose_args(&self, tail: &[&str]) -> Vec<String> {
        let mut args = vec![
            "compose".to_string(),
            "--project-directory".to_string(),
            self.install_dir.display().to_string(),
            "--env-file".to_string(),
            self.env_file().display().to_string(),
            "-f".to_string(),
            self.compose_file().display().to_string(),
        ];
        args.extend(tail.iter().map(|value| (*value).to_string()));
        args
    }

    pub(crate) fn compose_status(&self, tail: &[&str]) -> Result<()> {
        let args = self.compose_args(tail);
        let refs = args.iter().map(String::as_str).collect::<Vec<_>>();
        self.docker_status(&refs)
    }

    pub(crate) fn compose_output(&self, tail: &[&str]) -> Result<String> {
        let args = self.compose_args(tail);
        let refs = args.iter().map(String::as_str).collect::<Vec<_>>();
        self.docker_output(&refs)
    }

    pub(crate) fn compose_with_input(&self, tail: &[&str], input: &[u8]) -> Result<()> {
        let args = self.compose_args(tail);
        let refs = args.iter().map(String::as_str).collect::<Vec<_>>();
        let mut child = self
            .docker_command(&refs)
            .stdin(Stdio::piped())
            .stdout(if self.ui.verbose {
                Stdio::inherit()
            } else {
                Stdio::null()
            })
            .spawn()
            .context("run docker compose")?;
        child
            .stdin
            .as_mut()
            .context("open docker compose stdin")?
            .write_all(input)?;
        let status = child.wait()?;
        if !status.success() {
            bail!("docker compose failed with {status}");
        }
        Ok(())
    }

    pub(crate) fn load_control_config(
        &self,
        credentials: &RegistryCredentials,
    ) -> Result<ControlConfig> {
        let requested_version = env::var("ARGUS_VERSION").ok();
        let requested_domain = env::var("ARGUS_DOMAIN").ok();
        let requested_content_domain = env::var("ARGUS_CONTENT_DOMAIN").ok();
        let requested_user = env::var("ARGUS_BASIC_AUTH_USER").ok();
        let requested_password = env::var("ARGUS_BASIC_AUTH_PASSWORD").ok();
        let existing_values = if self.env_file().is_file() {
            read_env_file(&self.env_file())?
        } else {
            BTreeMap::new()
        };
        let cloudflare_credentials = self.config_dir.join("cloudflare.env");
        let saved_cloudflare_token = if cloudflare_credentials.is_file() {
            read_env_file(&cloudflare_credentials)?
                .get("ARGUS_CLOUDFLARE_API_TOKEN")
                .cloned()
        } else {
            None
        };
        let existing_install = !existing_values.is_empty();
        if existing_install {
            self.ui
                .detail("Existing Argus configuration found; preserving generated IDs and secrets");
        }
        let installed_version = existing_values.get("ARGUS_VERSION").cloned();
        let version = if existing_install {
            if let (Some(requested), Some(installed)) = (&requested_version, &installed_version) {
                if requested != installed {
                    self.ui.warning(&format!("Ignoring requested ARGUS_VERSION={requested}; use argusctl update for version changes"));
                }
            }
            installed_version.unwrap_or_else(|| {
                requested_version
                    .clone()
                    .unwrap_or_else(|| "main".to_string())
            })
        } else {
            requested_version.unwrap_or_else(|| "main".to_string())
        };
        let mut domain = requested_domain
            .or_else(|| existing_values.get("ARGUS_DOMAIN").cloned())
            .unwrap_or_default();
        if domain.is_empty() {
            domain = prompt_line("Primary Argus domain (for example argus.example.com): ")?;
        }
        domain.make_ascii_lowercase();
        validate_domain(&domain)?;
        let mut content_domain = requested_content_domain
            .or_else(|| existing_values.get("ARGUS_CONTENT_DOMAIN").cloned())
            .unwrap_or_else(|| format!("content.{domain}"));
        content_domain.make_ascii_lowercase();
        validate_domain(&content_domain)?;
        if content_domain == domain {
            bail!("Web and content domains must differ");
        }
        let acme_email = if let Some(value) = env::var("ARGUS_ACME_EMAIL")
            .ok()
            .or_else(|| existing_values.get("ARGUS_ACME_EMAIL").cloned())
        {
            value
        } else if lifecycle::interactive_available() {
            prompt_line("Certificate contact email: ")?
        } else if existing_install {
            "operator@argus.local".to_string()
        } else {
            bail!("ARGUS_ACME_EMAIL is required for an unattended control-plane install");
        };
        if !acme_email.contains('@')
            || acme_email.starts_with('@')
            || acme_email.ends_with('@')
            || acme_email.chars().any(char::is_whitespace)
        {
            bail!("certificate contact email is invalid");
        }
        let basic_auth_user = requested_user
            .or_else(|| existing_values.get("ARGUS_BASIC_AUTH_USER").cloned())
            .unwrap_or_else(|| "argus".to_string());
        validate_basic_user(&basic_auth_user)?;
        let (basic_auth_password, generated_basic_password) = if let Some(value) =
            requested_password.or_else(|| existing_values.get("ARGUS_BASIC_AUTH_PASSWORD").cloned())
        {
            (value, false)
        } else if lifecycle::interactive_available() {
            loop {
                let first = prompt_secret("Login password (Enter to generate): ")?;
                if first.is_empty() {
                    break (new_secret(24), true);
                }
                if first.len() < 12 {
                    self.ui
                        .warning("Login password must be at least 12 characters.");
                    continue;
                }
                let second = prompt_secret("Confirm login password: ")?;
                if first == second {
                    break (first, false);
                }
                self.ui.warning("Passwords do not match; try again.");
            }
        } else {
            (new_secret(24), true)
        };
        if !existing_install && !generated_basic_password && basic_auth_password.len() < 12 {
            bail!("ARGUS_BASIC_AUTH_PASSWORD must be at least 12 characters");
        }
        Ok(ControlConfig {
            registry: credentials.registry.clone(),
            version,
            domain,
            content_domain,
            basic_auth_user,
            basic_auth_password,
            postgres_password: value_or_secret(&existing_values, "ARGUS_POSTGRES_PASSWORD", 64),
            web_api_token: value_or_secret(&existing_values, "ARGUS_WEB_API_TOKEN", 64),
            worker_token: value_or_secret(&existing_values, "ARGUS_WORKER_TOKEN", 64),
            content_sync_token: value_or_secret(&existing_values, "ARGUS_CONTENT_SYNC_TOKEN", 64),
            payload_secret: value_or_secret(&existing_values, "PAYLOAD_SECRET", 64),
            org_id: value_or_uuid(&existing_values, "ARGUS_ORG_ID"),
            user_id: value_or_uuid(&existing_values, "ARGUS_USER_ID"),
            bootstrap_project_id: value_or_uuid(&existing_values, "ARGUS_BOOTSTRAP_PROJECT_ID"),
            bootstrap_environment_id: value_or_uuid(
                &existing_values,
                "ARGUS_BOOTSTRAP_ENVIRONMENT_ID",
            ),
            server_id: value_or_uuid(&existing_values, "ARGUS_SERVER_ID"),
            github_token: env::var("ARGUS_GITHUB_TOKEN")
                .ok()
                .or_else(|| existing_values.get("ARGUS_GITHUB_TOKEN").cloned())
                .unwrap_or_default(),
            rust_log: env::var("ARGUS_RUST_LOG")
                .ok()
                .or_else(|| existing_values.get("ARGUS_RUST_LOG").cloned())
                .unwrap_or_else(|| "info".to_string()),
            operator_email: env::var("ARGUS_OPERATOR_EMAIL")
                .unwrap_or_else(|_| "operator@argus.local".to_string()),
            acme_email,
            tls_mode: if existing_values.get("ARGUS_TLS_MODE").map(String::as_str)
                == Some("cloudflare-origin")
            {
                TlsMode::CloudflareOrigin
            } else {
                TlsMode::PublicAcme
            },
            cloudflare_api_token: env::var("ARGUS_CLOUDFLARE_API_TOKEN")
                .ok()
                .or(saved_cloudflare_token)
                .unwrap_or_default(),
            org_name: env::var("ARGUS_ORG_NAME").unwrap_or_else(|_| "Argus".to_string()),
            generated_basic_password,
            existing_install,
        })
    }

    pub(crate) fn pull_host_bundle(
        &self,
        config: &mut ControlConfig,
        include_deploy: bool,
    ) -> Result<()> {
        if config.existing_install && !is_revision(&config.version) {
            config.version = self.resolve_running_revision()?;
        }
        let requested = config.version.clone();
        let mut image = format!("{}/argus-host-tools:{requested}", config.registry);
        self.docker_status(&["pull", &image]).with_context(|| {
            format!("could not pull {image}; verify the PAT has read:packages access")
        })?;
        let resolved = self.docker_output(&[
            "image",
            "inspect",
            &image,
            "--format",
            "{{ index .Config.Labels \"org.opencontainers.image.revision\" }}",
        ])?;
        if !is_revision(&resolved) {
            bail!("{image} is missing an immutable Argus revision label");
        }
        if config.existing_install && is_revision(&requested) && resolved != requested {
            bail!(
                "installed revision {requested} resolved to unexpected artifact revision {resolved}"
            );
        }
        config.version = resolved;
        image = format!("{}/argus-host-tools:{}", config.registry, config.version);
        if requested != config.version {
            self.ui.detail(&format!(
                "Pinned requested version '{requested}' to {}",
                config.version
            ));
            self.docker_status(&["pull", &image])?;
        }
        self.install_host_tools(&image, include_deploy)
    }

    fn resolve_running_revision(&self) -> Result<String> {
        if !self.compose_file().is_file() {
            bail!("existing installation uses a mutable version but compose.yaml is missing");
        }
        let cid = self.compose_output(&["ps", "-q", "control-api"])?;
        if cid.is_empty() {
            bail!("existing installation uses a mutable version and control-api is not running");
        }
        let image_id = self.docker_output(&["inspect", "-f", "{{.Image}}", &cid])?;
        let revision = self.docker_output(&[
            "image",
            "inspect",
            &image_id,
            "--format",
            "{{ index .Config.Labels \"org.opencontainers.image.revision\" }}",
        ])?;
        if !is_revision(&revision) {
            bail!("could not recover immutable revision from the running Control API image");
        }
        Ok(revision)
    }

    pub(crate) fn install_host_tools(&self, image: &str, include_deploy: bool) -> Result<()> {
        fs::create_dir_all(&self.install_dir)?;
        fs::create_dir_all(&self.config_dir)?;
        fs::create_dir_all(self.state_dir.join("backups"))?;
        let tmp = temp_dir("argus-host-tools")?;
        let container = self.docker_output(&["create", image])?;
        let copy_result = (|| -> Result<()> {
            self.docker_status(&[
                "cp",
                &format!("{container}:/out/."),
                tmp.to_str().context("host tools temp path")?,
            ])?;
            if include_deploy {
                self.docker_status(&[
                    "cp",
                    &format!("{container}:/deploy/compose.yaml"),
                    self.compose_file().to_str().context("compose path")?,
                ])?;
                self.docker_status(&[
                    "cp",
                    &format!("{container}:/deploy/Caddyfile.template"),
                    self.install_dir
                        .join("Caddyfile.template")
                        .to_str()
                        .context("Caddy path")?,
                ])?;
            }
            self.docker_status(&[
                "cp",
                &format!("{container}:/deploy/systemd/argus-agent.service"),
                tmp.join("argus-agent.service")
                    .to_str()
                    .context("agent unit path")?,
            ])?;
            self.docker_status(&[
                "cp",
                &format!("{container}:/deploy/systemd/argus-helper.service"),
                tmp.join("argus-helper.service")
                    .to_str()
                    .context("helper unit path")?,
            ])?;
            for required in [
                "argus-agent",
                "argus-helper",
                "argusctl",
                "argus-installer",
                "argus-agent.service",
                "argus-helper.service",
            ] {
                if !tmp.join(required).is_file() {
                    bail!("host-tools image is incomplete: {required}");
                }
            }
            copy_file(
                &tmp.join("argus-agent"),
                Path::new("/usr/local/bin/argus-agent"),
                0o755,
            )?;
            copy_file(
                &tmp.join("argus-helper"),
                Path::new("/usr/local/bin/argus-helper"),
                0o755,
            )?;
            copy_file(
                &tmp.join("argusctl"),
                Path::new("/usr/local/bin/argusctl"),
                0o755,
            )?;
            copy_file(
                &tmp.join("argus-installer"),
                Path::new("/usr/local/bin/argus-installer"),
                0o755,
            )?;
            copy_file(
                &tmp.join("argus-agent.service"),
                Path::new("/etc/systemd/system/argus-agent.service"),
                0o644,
            )?;
            copy_file(
                &tmp.join("argus-helper.service"),
                Path::new("/etc/systemd/system/argus-helper.service"),
                0o644,
            )?;
            lifecycle::run_quiet("systemctl", &["daemon-reload"])?;
            Ok(())
        })();
        let _ = self.docker_status(&["rm", &container]);
        let _ = fs::remove_dir_all(&tmp);
        copy_result
    }
}
