use super::installer_shared::{ControlConfig, InstallMode, parse_os_release};
use anyhow::{Context, Result, bail};
use cli::lifecycle::{self, DEFAULT_INSTALL_DIR, DEFAULT_STATE_DIR, env_path};
use dialoguer::{Select, theme::ColorfulTheme};
use secrecy::ExposeSecret;
use std::{env, path::Path, process::Command};

const LOGO: &str = r#"    ___
   /   |  _________ ___  ______
  / /| | / ___/ __ `/ / / / __ \
 / ___ |/ /  / /_/ / /_/ / /_/ /
/_/  |_/_/   \__, /\__,_/\____/
            /____/"#;

#[derive(Debug, Clone)]
pub(crate) struct MenuItem {
    pub(crate) label: String,
    pub(crate) description: Option<String>,
}

impl MenuItem {
    pub(crate) fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            description: None,
        }
    }

    pub(crate) fn described(label: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            description: Some(description.into()),
        }
    }

    fn rendered(&self) -> String {
        match &self.description {
            Some(description) => format!("{}  —  {description}", self.label),
            None => self.label.clone(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MenuChoice {
    Selected(usize),
    Cancelled,
}

fn memory_label() -> Option<String> {
    let meminfo = std::fs::read_to_string("/proc/meminfo").ok()?;
    let kb = meminfo
        .lines()
        .find_map(|line| line.strip_prefix("MemTotal:"))?
        .split_whitespace()
        .next()?
        .parse::<f64>()
        .ok()?;
    Some(format!("{:.1} GB RAM", kb / 1024.0 / 1024.0))
}

fn host_name() -> Option<String> {
    let output = Command::new("hostname").output().ok()?;
    if !output.status.success() {
        return None;
    }
    let value = String::from_utf8_lossy(&output.stdout).trim().to_string();
    (!value.is_empty()).then_some(value)
}

fn short_version(value: &str) -> String {
    if value.len() == 40 && value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        value[..12].to_string()
    } else {
        value.to_string()
    }
}

fn welcome_header() -> String {
    let installer_version = env!("CARGO_PKG_VERSION");
    let revision = env::var("ARGUS_VERSION")
        .ok()
        .map(|value| short_version(&value));
    let os = parse_os_release()
        .ok()
        .and_then(|values| values.get("PRETTY_NAME").cloned())
        .unwrap_or_else(|| "Linux".to_string());
    let architecture = env::consts::ARCH;

    let mut host = vec![os, architecture.to_string()];
    if let Some(memory) = memory_label() {
        host.push(memory);
    }
    if let Some(name) = host_name() {
        host.push(name);
    }

    let build = revision
        .map(|value| format!(" · build {value}"))
        .unwrap_or_default();

    format!(
        "{LOGO}\n\nArgus Installer {installer_version}{build}\n{}\n",
        host.join(" · ")
    )
}

pub(crate) fn menu_select(
    header: Option<&str>,
    title: &str,
    items: &[MenuItem],
) -> Result<MenuChoice> {
    if items.is_empty() {
        bail!("menu has no options");
    }
    if !lifecycle::interactive_available() {
        bail!("interactive input is unavailable");
    }

    let term = lifecycle::interactive_term()?;
    if let Some(header) = header {
        term.write_line(header).context("render installer header")?;
    }
    let rendered = items.iter().map(MenuItem::rendered).collect::<Vec<_>>();
    let theme = ColorfulTheme::default();
    let selected = Select::with_theme(&theme)
        .with_prompt(title)
        .items(&rendered)
        .default(0)
        .report(false)
        .interact_on_opt(&term)
        .context("read installer selection")?;

    Ok(selected
        .map(MenuChoice::Selected)
        .unwrap_or(MenuChoice::Cancelled))
}

pub(crate) fn select_install_mode(requested: Option<String>) -> Result<InstallMode> {
    if let Some(value) = requested {
        let mode = InstallMode::parse(&value)?;
        if lifecycle::interactive_available() {
            println!("{}", welcome_header());
            println!(
                "Mode\n  {}\n",
                match mode {
                    InstallMode::ControlPlane => "Control Plane",
                    InstallMode::Agent => "Managed Server",
                    InstallMode::Repair => "Repair",
                    InstallMode::Update => "Update",
                    InstallMode::Uninstall => "Uninstall",
                }
            );
        }
        return Ok(mode);
    }

    if !lifecycle::interactive_available() {
        bail!("ARGUS_INSTALL_MODE is required in non-interactive mode");
    }

    let install_dir = env_path("ARGUS_INSTALL_DIR", DEFAULT_INSTALL_DIR);
    let state_dir = env_path("ARGUS_STATE_DIR", DEFAULT_STATE_DIR);
    let existing = install_dir.join(".env").is_file()
        || Path::new("/usr/local/bin/argus-agent").is_file()
        || state_dir.join("agent.json").is_file()
        || state_dir.join("uninstall-recovery").is_dir();
    let header = welcome_header();

    if existing {
        let items = [
            MenuItem::described("Repair", "Repair the existing Argus installation."),
            MenuItem::described("Update", "Update Argus on this server."),
            MenuItem::described("Uninstall", "Remove Argus from this server."),
        ];
        return match menu_select(Some(&header), "Existing Argus installation", &items)? {
            MenuChoice::Selected(0) => Ok(InstallMode::Repair),
            MenuChoice::Selected(1) => Ok(InstallMode::Update),
            MenuChoice::Selected(2) => Ok(InstallMode::Uninstall),
            MenuChoice::Selected(_) => unreachable!(),
            MenuChoice::Cancelled => bail!("operation cancelled"),
        };
    }

    let items = [
        MenuItem::described(
            "Control Plane",
            "Run the Argus dashboard, API and local agent on this server.",
        ),
        MenuItem::described(
            "Managed Server",
            "Connect this server to an existing Argus Control Plane.",
        ),
    ];
    match menu_select(Some(&header), "Choose installation type", &items)? {
        MenuChoice::Selected(0) => Ok(InstallMode::ControlPlane),
        MenuChoice::Selected(1) => Ok(InstallMode::Agent),
        MenuChoice::Selected(_) => unreachable!(),
        MenuChoice::Cancelled => bail!("installation cancelled"),
    }
}

pub(crate) fn print_control_success(config: &ControlConfig) {
    println!();
    println!("✓ Argus Control Plane installed\n");
    println!("  Dashboard    https://{}", config.domain);
    println!("  Content      https://{}", config.content_domain);
    println!("  Version      {}", short_version(&config.version));
    println!();
    println!("Administrator login");
    println!("  Username     {}", config.basic_auth_user);
    if config.generated_basic_password && lifecycle::interactive_available() {
        println!(
            "  Password     {}",
            config.basic_auth_password.expose_secret()
        );
        println!();
        println!("  Save these credentials somewhere secure.");
    } else {
        println!("  Password     configured");
    }
    println!();
    println!("  Credentials  sudo argusctl credentials");
    println!("  Status       sudo argusctl status");
    println!("  Update       sudo argusctl update");
}

pub(crate) fn print_managed_success() {
    println!();
    println!("✓ Managed Server connected\n");
    println!("  Status       sudo argusctl status");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commit_versions_are_shortened_for_display() {
        assert_eq!(
            short_version("0123456789abcdef0123456789abcdef01234567"),
            "0123456789ab"
        );
        assert_eq!(short_version("main"), "main");
    }

    #[test]
    fn menu_items_render_descriptions_without_terminal_escape_sequences() {
        let item = MenuItem::described("Control Plane", "Run Argus here.");
        assert_eq!(item.rendered(), "Control Plane  —  Run Argus here.");
        assert!(!item.rendered().contains("\x1b"));
    }
}
