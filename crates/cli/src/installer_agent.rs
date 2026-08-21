use super::installer_shared::{Installer, is_revision};
use anyhow::{Context, Result, bail};
use cli::lifecycle::{self, RegistryCredentials, prompt_secret};
use serde_json::Value;
use std::{
    env, fs,
    io::Write,
    process::{Command, Stdio},
    thread,
    time::Duration,
};

pub(crate) struct ManagedNodeConfig {
    control_plane_url: String,
    server_id: String,
    enrollment_token: String,
}

impl Installer {
    pub(crate) fn collect_managed_node_config(&self) -> Result<ManagedNodeConfig> {
        let setup_code = match env::var("ARGUS_SETUP_CODE")
            .ok()
            .filter(|value| !value.is_empty())
        {
            Some(value) => value,
            None => prompt_secret("Argus setup code: ")?,
        };
        let decoded = Command::new("base64")
            .arg("-d")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .and_then(|mut child| {
                child
                    .stdin
                    .as_mut()
                    .unwrap()
                    .write_all(setup_code.as_bytes())?;
                child.wait_with_output()
            })
            .context("decode setup code")?;
        if !decoded.status.success() {
            bail!("invalid setup code");
        }
        let setup: Value = serde_json::from_slice(&decoded.stdout).context("invalid setup code")?;
        let control_plane_url = setup
            .get("control_plane_url")
            .and_then(Value::as_str)
            .context("setup code is missing control_plane_url")?
            .to_string();
        let server_id = setup
            .get("server_id")
            .and_then(Value::as_str)
            .context("setup code is missing server_id")?
            .to_string();
        let enrollment_token = setup
            .get("enrollment_token")
            .and_then(Value::as_str)
            .context("setup code is missing enrollment_token")?
            .to_string();
        if !control_plane_url.starts_with("https://")
            && env::var("ARGUS_ALLOW_INSECURE_CONTROL_PLANE").as_deref() != Ok("1")
        {
            bail!("remote control plane must use HTTPS");
        }
        Ok(ManagedNodeConfig {
            control_plane_url,
            server_id,
            enrollment_token,
        })
    }

    pub(crate) fn install_managed_node(
        &self,
        credentials: &RegistryCredentials,
        setup: &ManagedNodeConfig,
    ) -> Result<()> {
        let ManagedNodeConfig {
            control_plane_url,
            server_id,
            enrollment_token,
        } = setup;

        let requested = env::var("ARGUS_VERSION").unwrap_or_else(|_| "main".to_string());
        let initial_image = format!("{}/argus-host-tools:{requested}", credentials.registry);
        self.docker_status(&["pull", &initial_image])?;
        let revision = self.docker_output(&[
            "image",
            "inspect",
            &initial_image,
            "--format",
            "{{ index .Config.Labels \"org.opencontainers.image.revision\" }}",
        ])?;
        if !is_revision(&revision) {
            bail!("host-tools image is missing an immutable revision label");
        }
        let image = format!("{}/argus-host-tools:{revision}", credentials.registry);
        if requested != revision {
            self.docker_status(&["pull", &image])?;
        }

        self.ensure_argus_user()?;
        self.install_host_tools(&image, false)?;
        self.write_helper_env()?;
        self.write_agent_env(
            control_plane_url,
            server_id,
            &env::var("ARGUS_RUST_LOG").unwrap_or_else(|_| "info".to_string()),
            Some(enrollment_token),
        )?;
        lifecycle::run_quiet(
            "systemctl",
            &[
                "enable",
                "--now",
                "argus-helper.service",
                "argus-agent.service",
            ],
        )?;

        let agent_json = self.state_dir.join("agent.json");
        for _ in 0..60 {
            if agent_json.is_file() && fs::metadata(&agent_json)?.len() > 0 {
                break;
            }
            thread::sleep(Duration::from_secs(2));
        }
        if !agent_json.is_file() || fs::metadata(&agent_json)?.len() == 0 {
            bail!("Argus Agent did not enroll successfully");
        }

        self.write_agent_env(
            control_plane_url,
            server_id,
            &env::var("ARGUS_RUST_LOG").unwrap_or_else(|_| "info".to_string()),
            None,
        )?;
        lifecycle::run_quiet("systemctl", &["restart", "argus-agent.service"])?;
        fs::write(self.config_dir.join("revision"), format!("{revision}\n"))?;
        lifecycle::run_quiet(
            "systemctl",
            &["is-active", "--quiet", "argus-helper.service"],
        )?;
        lifecycle::run_quiet(
            "systemctl",
            &["is-active", "--quiet", "argus-agent.service"],
        )?;
        println!();
        self.ui.success_title("Argus managed node is connected");
        println!("Control plane: {control_plane_url}");
        println!("Server ID:     {server_id}");
        println!("Revision:      {revision}");
        Ok(())
    }
}
