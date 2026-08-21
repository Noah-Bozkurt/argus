use anyhow::Result;
use clap::{Parser, Subcommand};
use cli::lifecycle;

mod installer_control;
mod installer_host;
mod installer_shared;

use installer_shared::{select_mode, InstallMode, Installer};

#[derive(Debug, Parser)]
#[command(
    name = "argus-installer",
    about = "Install an Argus control plane or managed node"
)]
struct Cli {
    #[arg(long, env = "ARGUS_INSTALL_MODE")]
    mode: Option<String>,
    #[arg(long, short = 'v')]
    verbose: bool,
    #[command(subcommand)]
    action: Option<Action>,
}

#[derive(Debug, Subcommand)]
enum Action {
    RegistryLogin {
        #[arg(long)]
        username: Option<String>,
    },
    Uninstall {
        #[arg(long)]
        yes: bool,
        #[arg(long)]
        purge_data: bool,
    },
}

impl Installer {
    fn run(&mut self) -> Result<()> {
        self.ui.title();
        self.ui
            .working("Checking host requirements", || self.preflight())?;

        let credentials = self.ui.working("Authenticating with the Argus registry", || {
            self.authenticate_registry()
        })?;

        if self.mode == InstallMode::Agent {
            self.ui.working("Installing managed-node bundle", || {
                self.install_managed_node(&credentials)
            })?;
            return Ok(());
        }

        let mut config = self
            .ui
            .working("Collecting control-plane configuration", || {
                self.load_control_config(&credentials)
            })?;
        self.ui.detail(&format!(
            "Installing Argus for {}",
            config.get("ARGUS_DOMAIN")?
        ));

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
        })?;
        self.print_summary(&config)?;
        Ok(())
    }
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.action {
        Some(Action::RegistryLogin { username }) => lifecycle::registry_login(username.as_deref()),
        Some(Action::Uninstall { yes, purge_data }) => {
            lifecycle::uninstall(lifecycle::UninstallOptions::from_env(yes, purge_data))
        }
        None => {
            let mode = select_mode(cli.mode)?;
            let mut installer = Installer::new(mode, cli.verbose)?;
            installer.run().map_err(|error| {
                eprintln!("\n  ✗ Installation failed: {error:#}");
                if !cli.verbose {
                    eprintln!("    Re-run with --verbose for full diagnostics.");
                }
                error
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::installer_shared::{is_revision, InstallMode};

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
}
