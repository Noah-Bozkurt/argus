use super::installer_shared::{ControlConfig, Installer};
use anyhow::{Context, Result, bail};
use cli::lifecycle;
use std::{fs, os::unix::fs::PermissionsExt, path::PathBuf, process::Command};

struct FileSnapshot {
    entries: Vec<SnapshotEntry>,
}

struct SnapshotEntry {
    path: PathBuf,
    value: Option<(Vec<u8>, u32)>,
}

impl FileSnapshot {
    fn capture(paths: Vec<PathBuf>) -> Result<Self> {
        let mut entries = Vec::new();
        for path in paths {
            let value = if path.is_file() {
                let metadata = fs::metadata(&path)?;
                Some((fs::read(&path)?, metadata.permissions().mode() & 0o777))
            } else {
                None
            };
            entries.push(SnapshotEntry { path, value });
        }
        Ok(Self { entries })
    }

    fn restore(&self) -> Result<()> {
        for entry in &self.entries {
            match &entry.value {
                Some((data, mode)) => {
                    if let Some(parent) = entry.path.parent() {
                        fs::create_dir_all(parent)?;
                    }
                    fs::write(&entry.path, data)?;
                    fs::set_permissions(&entry.path, fs::Permissions::from_mode(*mode))?;
                }
                None if entry.path.exists() => lifecycle::remove_path(&entry.path)?,
                None => {}
            }
        }
        Ok(())
    }
}

impl Installer {
    fn restore_uninstall_recovery(&self) -> Result<()> {
        let recovery = self.state_dir.join("uninstall-recovery");
        if self.env_file().is_file() || !recovery.is_dir() {
            return Ok(());
        }
        for (name, target) in [
            ("runtime.env", self.install_dir.join(".env")),
            ("compose.yaml", self.install_dir.join("compose.yaml")),
            ("Caddyfile", self.install_dir.join("Caddyfile")),
            ("registry.env", self.config_dir.join("registry.env")),
            ("agent.env", self.config_dir.join("agent.env")),
            ("helper.env", self.config_dir.join("helper.env")),
            ("revision", self.config_dir.join("revision")),
        ] {
            let source = recovery.join(name);
            if source.is_file() {
                lifecycle::copy_file(&source, &target, 0o600)?;
            }
        }
        Ok(())
    }

    fn repair_paths(&self) -> Vec<PathBuf> {
        vec![
            self.install_dir.join(".env"),
            self.install_dir.join("compose.yaml"),
            self.install_dir.join("Caddyfile"),
            self.install_dir.join("Caddyfile.template"),
            self.config_dir.join("registry.env"),
            self.config_dir.join("agent.env"),
            self.config_dir.join("helper.env"),
            self.config_dir.join("revision"),
            PathBuf::from("/usr/local/bin/argus-agent"),
            PathBuf::from("/usr/local/bin/argus-helper"),
            PathBuf::from("/usr/local/bin/argusctl"),
            PathBuf::from("/usr/local/bin/argus-installer"),
            PathBuf::from("/etc/systemd/system/argus-agent.service"),
            PathBuf::from("/etc/systemd/system/argus-helper.service"),
        ]
    }

    fn repair_control_plane(&self, config: &mut ControlConfig) -> Result<()> {
        self.pull_host_bundle(config, true)?;
        self.ensure_argus_user()?;
        self.write_runtime_env(config)?;
        self.regenerate_caddy_config(config)?;
        self.start_control_plane()?;
        self.enroll_local_agent(config)?;
        self.verify_installation(config)
    }

    fn repair_managed_node(&self, credentials: &lifecycle::RegistryCredentials) -> Result<()> {
        let agent_config = self.state_dir.join("agent.json");
        if !agent_config.is_file() {
            bail!(
                "managed-node identity is missing; generate a new setup code and re-enroll this host"
            );
        }
        for path in [
            self.config_dir.join("agent.env"),
            self.config_dir.join("helper.env"),
        ] {
            if !path.is_file() {
                bail!(
                    "{} is missing; restore retained recovery configuration or re-enroll this host",
                    path.display()
                );
            }
        }
        let revision = fs::read_to_string(self.config_dir.join("revision"))
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .context(
                "installed managed-node revision is unknown; re-enroll this legacy node instead of guessing a version",
            )?;
        let image = format!("{}/argus-host-tools:{revision}", credentials.registry);
        self.docker_status(&["pull", &image])?;
        self.ensure_argus_user()?;
        self.install_host_tools(&image, false)?;
        lifecycle::run_quiet("systemctl", &["daemon-reload"])?;
        lifecycle::run_quiet(
            "systemctl",
            &[
                "enable",
                "--now",
                "argus-helper.service",
                "argus-agent.service",
            ],
        )?;
        lifecycle::run_quiet(
            "systemctl",
            &["is-active", "--quiet", "argus-helper.service"],
        )?;
        lifecycle::run_quiet(
            "systemctl",
            &["is-active", "--quiet", "argus-agent.service"],
        )
    }

    pub(crate) fn repair_installation(&self) -> Result<()> {
        self.restore_uninstall_recovery()
            .context("restore retained uninstall configuration")?;
        let credentials = lifecycle::collect_registry_credentials(&self.config_dir, None)?;
        self.ui
            .working("Checking host requirements", || self.preflight())?;
        self.ui
            .working("Authenticating with the Argus registry", || {
                lifecycle::docker_login(&credentials, &self.docker_config)?;
                lifecycle::save_registry_credentials(&self.config_dir, &credentials)
            })?;
        let snapshot = FileSnapshot::capture(self.repair_paths())?;
        let result = if self.env_file().is_file() {
            let mut config = self.load_control_config(&credentials)?;
            if !config.existing_install {
                bail!("no existing control-plane installation was found");
            }
            self.ui
                .working("Repairing Argus", || self.repair_control_plane(&mut config))
        } else {
            self.ui.working("Repairing Argus managed node", || {
                self.repair_managed_node(&credentials)
            })
        };
        if let Err(error) = result {
            self.ui
                .warning("Repair failed; restoring the previous installation files.");
            snapshot
                .restore()
                .context("repair failed and previous files could not be fully restored")?;
            let _ = Command::new("systemctl").arg("daemon-reload").status();
            let _ = self.compose_status(&["up", "-d"]);
            let _ = Command::new("systemctl")
                .args([
                    "enable",
                    "--now",
                    "argus-helper.service",
                    "argus-agent.service",
                ])
                .status();
            return Err(error.context("repair failed; previous installation files were restored"));
        }
        println!();
        self.ui.success_title("Argus repair completed");
        self.ui.detail("Run argusctl doctor to review the host.");
        Ok(())
    }
}
