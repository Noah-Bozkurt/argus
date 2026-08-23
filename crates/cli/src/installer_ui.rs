use super::installer_shared::{ControlConfig, InstallMode, parse_os_release};
use anyhow::{Context, Result, bail};
use cli::lifecycle::{
    self, DEFAULT_INSTALL_DIR, DEFAULT_STATE_DIR, env_path, prompt_line,
};
use std::{
    env,
    fs::{File, OpenOptions},
    io::{Read, Write},
    path::Path,
    process::{Command, Stdio},
};

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

    pub(crate) fn described(
        label: impl Into<String>,
        description: impl Into<String>,
    ) -> Self {
        Self {
            label: label.into(),
            description: Some(description.into()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MenuChoice {
    Selected(usize),
    Cancelled,
}

struct TerminalMode {
    tty: File,
    original: String,
    color: bool,
}

impl TerminalMode {
    fn enter() -> Result<Self> {
        let tty = OpenOptions::new().read(true).write(true).open("/dev/tty")?;
        let output = Command::new("stty")
            .arg("-g")
            .stdin(Stdio::from(tty.try_clone()?))
            .output()
            .context("read terminal mode")?;
        if !output.status.success() {
            bail!("could not read terminal mode");
        }
        let original = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let status = Command::new("stty")
            .args(["-echo", "-icanon", "min", "1", "time", "0"])
            .stdin(Stdio::from(tty.try_clone()?))
            .status()
            .context("enable terminal navigation")?;
        if !status.success() {
            bail!("could not enable terminal navigation");
        }
        Ok(Self {
            tty,
            original,
            color: env::var_os("NO_COLOR").is_none(),
        })
    }

    fn paint(&self, code: &str, text: &str) -> String {
        if self.color {
            format!("\x1b[{code}m{text}\x1b[0m")
        } else {
            text.to_string()
        }
    }
}

impl Drop for TerminalMode {
    fn drop(&mut self) {
        let _ = Command::new("stty")
            .arg(&self.original)
            .stdin(
                self.tty
                    .try_clone()
                    .map(Stdio::from)
                    .unwrap_or(Stdio::null()),
            )
            .status();
        let _ = write!(self.tty, "\x1b[?25h");
        let _ = self.tty.flush();
    }
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
    let revision = env::var("ARGUS_VERSION").ok().map(|value| short_version(&value));
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

    match menu_select_terminal(header, title, items) {
        Ok(choice) => Ok(choice),
        Err(_) => menu_select_fallback(header, title, items),
    }
}

fn menu_select_terminal(
    header: Option<&str>,
    title: &str,
    items: &[MenuItem],
) -> Result<MenuChoice> {
    let mut terminal = TerminalMode::enter()?;
    let mut selected = 0usize;

    loop {
        write!(terminal.tty, "\x1b[?25l\x1b[2J\x1b[H")?;
        if let Some(header) = header {
            writeln!(terminal.tty, "{header}")?;
        }
        writeln!(terminal.tty, "{}\n", terminal.paint("1", title))?;

        let has_descriptions = items.iter().any(|item| item.description.is_some());
        for (index, item) in items.iter().enumerate() {
            if index == selected {
                writeln!(
                    terminal.tty,
                    "{}",
                    terminal.paint("1;36", &format!("  › {}", item.label))
                )?;
            } else {
                writeln!(terminal.tty, "    {}", item.label)?;
            }
            if let Some(description) = &item.description {
                writeln!(
                    terminal.tty,
                    "{}",
                    terminal.paint("2", &format!("      {description}"))
                )?;
            }
            if has_descriptions {
                writeln!(terminal.tty)?;
            }
        }

        writeln!(
            terminal.tty,
            "{}",
            terminal.paint("2", "↑/↓ Select   Enter Continue   q Cancel")
        )?;
        terminal.tty.flush()?;

        let mut byte = [0u8; 1];
        terminal.tty.read_exact(&mut byte)?;
        match byte[0] {
            b'\r' | b'\n' => return Ok(MenuChoice::Selected(selected)),
            b'q' | b'Q' => return Ok(MenuChoice::Cancelled),
            0x1b => {
                let mut sequence = [0u8; 2];
                if terminal.tty.read_exact(&mut sequence).is_ok() && sequence[0] == b'[' {
                    match sequence[1] {
                        b'A' => selected = selected.checked_sub(1).unwrap_or(items.len() - 1),
                        b'B' => selected = (selected + 1) % items.len(),
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
}

fn menu_select_fallback(
    header: Option<&str>,
    title: &str,
    items: &[MenuItem],
) -> Result<MenuChoice> {
    if let Some(header) = header {
        println!("{header}");
    }
    println!("{title}\n");
    for (index, item) in items.iter().enumerate() {
        println!("  {}. {}", index + 1, item.label);
        if let Some(description) = &item.description {
            println!("     {description}");
        }
    }
    let answer = prompt_line(&format!("\nChoose [1-{}] or q: ", items.len()))?;
    if answer.eq_ignore_ascii_case("q") {
        return Ok(MenuChoice::Cancelled);
    }
    let selected = answer
        .parse::<usize>()
        .with_context(|| format!("enter a number from 1 to {}", items.len()))?;
    if !(1..=items.len()).contains(&selected) {
        bail!("enter a number from 1 to {}", items.len());
    }
    Ok(MenuChoice::Selected(selected - 1))
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
        println!("  Password     {}", config.basic_auth_password);
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
    fn menu_items_can_include_descriptions() {
        let item = MenuItem::described("Control Plane", "Run Argus here.");
        assert_eq!(item.label, "Control Plane");
        assert_eq!(item.description.as_deref(), Some("Run Argus here."));
    }
}