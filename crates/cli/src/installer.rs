use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};
use cli::{domain::require_domain_resolution, lifecycle};

mod installer_agent;
mod installer_control;
mod installer_host;
mod installer_repair;
mod installer_review;
mod installer_shared;

use installer_review::review_control_install;
use installer_shared::{ControlConfig, InstallMode, Installer, select_mode, validate_domain};

#[derive(Debug, Parser)]
#[command(
    name = "argus-installer",
    about = "Install an Argus control plane or managed node"
)]
struct Cli {
    /// Installation path: control-plane, agent, repair, update, or uninstall.
    #[arg(long, env = "ARGUS_INSTALL_MODE")]
    mode: Option<String>,
    /// Show detailed, secret-safe diagnostics.
    #[arg(long, short = 'v')]
    verbose: bool,
    #[command(subcommand)]
    action: Option<Action>,
}

#[derive(Debug, Subcommand)]
enum Action {
    /// Validate and store GHCR credentials for future lifecycle operations.
    RegistryLogin {
        /// GitHub username; the token is entered securely.
        #[arg(long)]
        username: Option<String>,
    },
    /// Remove Argus, preserving persistent data unless purge is requested.
    Uninstall {
        /// Skip interactive confirmation.
        #[arg(long)]
        yes: bool,
        /// Permanently remove data, backups, logs, and Docker volumes.
        #[arg(long)]
        purge_data: bool,
    },
}

fn resolve_content_domain_input(main_domain: &str, default: &str, entered: &str) -> Result<String> {
    let content_domain = if entered.trim().is_empty() {
        default.to_string()
    } else {
        entered.trim().to_ascii_lowercase()
    };
    validate_domain(&content_domain)?;
    if content_domain == main_domain {
        bail!("Web and content domains must differ");
    }
    Ok(content_domain)
}

fn uninstall_confirmed(answer: &str) -> bool {
    answer == "YES"
}

fn purge_data_from_answer(answer: &str) -> bool {
    matches!(answer.trim().to_ascii_lowercase().as_str(), "y" | "yes")
}

fn run_uninstall(yes: bool, mut purge_data: bool) -> Result<()> {
    if !yes {
        if !lifecycle::interactive_available() {
            bail!("confirmation required; rerun with --yes");
        }

        println!("Argus uninstall\n");
        println!("  This will stop Argus and remove its binaries and configuration.");
        println!("  State and Docker volumes can be preserved for recovery.\n");

        if purge_data {
            println!("  WARNING: Purging permanently deletes all Argus data, backups, logs,");
            println!("  and Docker volumes. This cannot be undone without an external backup.\n");
        }

        let answer = lifecycle::prompt_line("Type YES to continue: ")?;
        if !uninstall_confirmed(&answer) {
            bail!("uninstall cancelled");
        }

        if !purge_data {
            println!();
            println!("  WARNING: Purging permanently deletes all Argus data, backups, logs,");
            println!("  and Docker volumes. This cannot be undone without an external backup.");
            let answer = lifecycle::prompt_line("Purge all Argus data? [y/N]: ")?;
            purge_data = purge_data_from_answer(&answer);
        }
    }

    lifecycle::uninstall(lifecycle::UninstallOptions::from_env(true, purge_data))
}

impl Installer {
    fn prompt_content_domain(&self, config: &mut ControlConfig) -> Result<()> {
        if config.existing_install
            || std::env::var_os("ARGUS_CONTENT_DOMAIN").is_some()
            || !lifecycle::interactive_available()
        {
            return Ok(());
        }

        let default = config.content_domain.clone();
        let entered = lifecycle::prompt_line(&format!("Content domain [{default}]: "))?;
        config.content_domain = resolve_content_domain_input(&config.domain, &default, &entered)?;
        Ok(())
    }

    fn run(&mut self) -> Result<()> {
        lifecycle::require_root().context("installer must run as root")?;
        self.ui.enable_log(&self.log_dir)?;
        self.ui.title();

        if self.mode == InstallMode::Repair {
            return self.repair_installation();
        }
        if self.mode == InstallMode::Update {
            return lifecycle::run("/usr/local/bin/argusctl", &["update"])
                .context("start Argus update");
        }
        if self.mode == InstallMode::Uninstall {
            return run_uninstall(false, false);
        }

        if self.mode == InstallMode::ControlPlane && self.env_file().is_file() {
            bail!(
                "an Argus control plane is already installed; use repair, update, or a dedicated configuration command"
            );
        }
        if self.mode == InstallMode::Agent && self.state_dir.join("agent.json").is_file() {
            bail!("this server is already enrolled; use repair instead of reinstalling it");
        }

        if self.mode == InstallMode::Agent {
            let credentials = lifecycle::collect_registry_credentials(&self.config_dir, None)?;
            let setup = self.collect_managed_node_config()?;
            let fresh_install = !self.state_dir.join("agent.json").exists()
                && !self.env_file().exists()
                && !std::path::Path::new("/usr/local/bin/argus-agent").exists();
            let result = (|| -> Result<()> {
                self.ui
                    .working("Checking host requirements", || self.preflight())?;
                self.ui
                    .working("Authenticating with the Argus registry", || {
                        lifecycle::docker_login(&credentials, &self.docker_config)?;
                        lifecycle::save_registry_credentials(&self.config_dir, &credentials)
                    })?;
                self.ui.working("Installing managed-node bundle", || {
                    self.install_managed_node(&credentials, &setup)
                })
            })();
            if let Err(error) = result {
                if fresh_install {
                    self.ui.warning(
                        "Managed-node installation failed; removing installed Argus components.",
                    );
                    if let Err(cleanup_error) =
                        lifecycle::uninstall(lifecycle::UninstallOptions::from_env(true, true))
                    {
                        return Err(error.context(format!(
                            "automatic rollback also failed: {cleanup_error:#}"
                        )));
                    }
                    self.ui.warning("Automatic rollback completed.");
                }
                return Err(error);
            }
            return Ok(());
        }

        // Collect control-plane inputs before host preflight so DNS can fail before apt,
        // Docker setup, registry credential storage, or Argus deployment mutates the host.
        let mut credentials = lifecycle::collect_registry_credentials(&self.config_dir, None)?;
        let mut config = self.load_control_config(&credentials)?;
        self.prompt_content_domain(&mut config)?;
        review_control_install(&mut credentials, &mut config)?;
        config.registry.clone_from(&credentials.registry);
        self.ui.working("Checking DNS resolution", || {
            require_domain_resolution(&config.domain)?;
            require_domain_resolution(&config.content_domain)
        })?;

        let install_result = (|| -> Result<()> {
            self.ui
                .working("Checking host requirements", || self.preflight())?;
            self.ui
                .working("Authenticating with the Argus registry", || {
                    lifecycle::docker_login(&credentials, &self.docker_config)?;
                    lifecycle::save_registry_credentials(&self.config_dir, &credentials)
                })?;
            self.ui
                .detail(&format!("Installing Argus for {}", config.domain));

            self.ui.working("Downloading and verifying Argus", || {
                self.pull_host_bundle(&mut config, true)
            })?;
            self.ui.working("Configuring Argus services", || {
                self.ensure_argus_user()?;
                self.write_runtime_env(&config)?;
                self.generate_caddy_config(&config)
            })?;
            self.ui.working("Starting the control plane", || {
                self.start_control_plane()?;
                self.bootstrap_control_plane(&config)?;
                self.enroll_local_agent(&config)
            })?;
            self.ui.working("Verifying installation health", || {
                self.verify_installation(&config)
            })
        })();

        if let Err(error) = install_result {
            if !config.existing_install {
                self.ui
                    .warning("Fresh installation failed; removing installed Argus components.");
                if let Err(cleanup_error) =
                    lifecycle::uninstall(lifecycle::UninstallOptions::from_env(true, true))
                {
                    return Err(
                        error.context(format!("automatic rollback also failed: {cleanup_error:#}"))
                    );
                }
                self.ui.warning("Automatic rollback completed.");
            }
            return Err(error);
        }

        self.print_summary(&config);
        Ok(())
    }
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.action {
        Some(Action::RegistryLogin { username }) => lifecycle::registry_login(username.as_deref()),
        Some(Action::Uninstall { yes, purge_data }) => run_uninstall(yes, purge_data),
        None => {
            let mode = select_mode(cli.mode)?;
            let mut installer = Installer::new(mode, cli.verbose)?;
            installer.run().map_err(|error| {
                installer
                    .ui
                    .record(&format!("INSTALLATION FAILED: {error:#}"));
                eprintln!("\n  ✗ Installation failed: {error:#}");
                if !cli.verbose {
                    eprintln!("    Re-run with --verbose for full diagnostics.");
                }
                let log = installer.log_dir.join("installer.log");
                if log.is_file() {
                    eprintln!("    Redacted log: {}", log.display());
                }
                error
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        installer_shared::{InstallMode, is_revision},
        purge_data_from_answer, resolve_content_domain_input, uninstall_confirmed,
    };

    #[test]
    fn revision_validation_is_strict() {
        assert!(is_revision("0123456789abcdef0123456789abcdef01234567"));
        assert!(!is_revision("main"));
        assert!(!is_revision("0123456789ABCDEF0123456789ABCDEF01234567"));
    }

    #[test]
    fn supported_modes_are_explicit() {
        assert_eq!(
            InstallMode::parse("control-plane").unwrap(),
            InstallMode::ControlPlane
        );
        assert_eq!(InstallMode::parse("agent").unwrap(), InstallMode::Agent);
        assert!(InstallMode::parse("server").is_err());
    }

    #[test]
    fn empty_content_domain_input_keeps_content_subdomain_default() {
        assert_eq!(
            resolve_content_domain_input("argus.example.com", "content.argus.example.com", "")
                .unwrap(),
            "content.argus.example.com"
        );
    }

    #[test]
    fn custom_content_domain_is_normalized_and_must_differ_from_web_domain() {
        assert_eq!(
            resolve_content_domain_input(
                "argus.example.com",
                "content.argus.example.com",
                "CMS.EXAMPLE.COM"
            )
            .unwrap(),
            "cms.example.com"
        );
        assert!(
            resolve_content_domain_input(
                "argus.example.com",
                "content.argus.example.com",
                "argus.example.com"
            )
            .is_err()
        );
    }

    #[test]
    fn uninstall_requires_literal_uppercase_yes() {
        assert!(uninstall_confirmed("YES"));
        for answer in ["", "yes", "Yes", " YES ", "Y"] {
            assert!(!uninstall_confirmed(answer), "{answer}");
        }
    }

    #[test]
    fn uninstall_purge_prompt_defaults_to_preserving_data() {
        for answer in ["", "n", "N", "no", "anything else"] {
            assert!(!purge_data_from_answer(answer), "{answer}");
        }
        for answer in ["y", "Y", "yes", "YES", " yes "] {
            assert!(purge_data_from_answer(answer), "{answer}");
        }
    }
}
