use anyhow::{Result, bail};
use cli::lifecycle::{
    self, DEFAULT_CONFIG_DIR, DEFAULT_INSTALL_DIR, DEFAULT_LOG_DIR, DEFAULT_STATE_DIR, env_path,
    prompt_line, temp_dir,
};
use std::{
    collections::BTreeMap,
    env, fs,
    io::{IsTerminal, Write},
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::Duration,
};
use uuid::Uuid;

#[derive(Clone)]
pub(crate) struct Ui {
    color: bool,
    pub(crate) verbose: bool,
}

impl Ui {
    pub(crate) fn new(verbose: bool) -> Self {
        Self {
            color: std::io::stdout().is_terminal() && env::var_os("NO_COLOR").is_none(),
            verbose,
        }
    }

    fn paint(&self, code: &str, text: &str) -> String {
        if self.color {
            format!("\x1b[{code}m{text}\x1b[0m")
        } else {
            text.to_string()
        }
    }

    pub(crate) fn title(&self) {
        println!("{}\n", self.paint("1;36", "Argus installer"));
    }
    pub(crate) fn detail(&self, message: &str) {
        println!("{}", self.paint("2", &format!("    {message}")));
    }
    pub(crate) fn warning(&self, message: &str) {
        eprintln!("{} {message}", self.paint("33", "  !"));
    }
    pub(crate) fn success_title(&self, message: &str) {
        println!("{}", self.paint("1;32", message));
    }

    pub(crate) fn working<T>(&self, message: &str, work: impl FnOnce() -> Result<T>) -> Result<T> {
        if self.verbose || !std::io::stdout().is_terminal() {
            println!("{} {message}", self.paint("36", "  ›"));
            let result = work();
            if result.is_ok() {
                println!("{} {message}", self.paint("32", "  ✓"));
            }
            return result;
        }
        let running = Arc::new(AtomicBool::new(true));
        let flag = Arc::clone(&running);
        let message_owned = message.to_string();
        let color = self.color;
        let spinner = thread::spawn(move || {
            let frames = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
            let mut i = 0usize;
            while flag.load(Ordering::Relaxed) {
                let frame = frames[i % frames.len()];
                let prefix = if color {
                    format!("\x1b[36m  {frame}\x1b[0m")
                } else {
                    format!("  {frame}")
                };
                print!("\r{prefix} {message_owned}\x1b[K");
                let _ = std::io::stdout().flush();
                i += 1;
                thread::sleep(Duration::from_millis(90));
            }
        });
        let result = work();
        running.store(false, Ordering::Relaxed);
        let _ = spinner.join();
        print!("\r\x1b[2K");
        let _ = std::io::stdout().flush();
        if result.is_ok() {
            println!("{} {message}", self.paint("32", "  ✓"));
        }
        result
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum InstallMode {
    ControlPlane,
    Agent,
}

impl InstallMode {
    pub(crate) fn parse(value: &str) -> Result<Self> {
        match value {
            "control-plane" => Ok(Self::ControlPlane),
            "agent" => Ok(Self::Agent),
            _ => bail!("ARGUS_INSTALL_MODE must be control-plane or agent"),
        }
    }
}

pub(crate) fn select_mode(requested: Option<String>) -> Result<InstallMode> {
    if let Some(value) = requested {
        return InstallMode::parse(&value);
    }
    if !lifecycle::interactive_available() {
        bail!("ARGUS_INSTALL_MODE must be control-plane or agent in non-interactive mode");
    }
    println!("  1. Install an Argus control plane here.");
    println!("  2. Connect this server to an existing Argus instance.\n");
    match prompt_line("Choose [1-2]: ")?.as_str() {
        "1" => Ok(InstallMode::ControlPlane),
        "2" => Ok(InstallMode::Agent),
        _ => bail!("invalid installation mode"),
    }
}

#[derive(Debug, Clone)]
pub(crate) struct ControlConfig {
    pub(crate) registry: String,
    pub(crate) version: String,
    pub(crate) domain: String,
    pub(crate) content_domain: String,
    pub(crate) basic_auth_user: String,
    pub(crate) basic_auth_password: String,
    pub(crate) postgres_password: String,
    pub(crate) web_api_token: String,
    pub(crate) worker_token: String,
    pub(crate) content_sync_token: String,
    pub(crate) payload_secret: String,
    pub(crate) org_id: String,
    pub(crate) user_id: String,
    pub(crate) bootstrap_project_id: String,
    pub(crate) bootstrap_environment_id: String,
    pub(crate) server_id: String,
    pub(crate) github_token: String,
    pub(crate) rust_log: String,
    pub(crate) operator_email: String,
    pub(crate) org_name: String,
    pub(crate) generated_basic_password: bool,
    pub(crate) existing_install: bool,
}

pub(crate) struct Installer {
    pub(crate) ui: Ui,
    pub(crate) mode: InstallMode,
    pub(crate) install_dir: PathBuf,
    pub(crate) config_dir: PathBuf,
    pub(crate) state_dir: PathBuf,
    pub(crate) log_dir: PathBuf,
    pub(crate) docker_config: PathBuf,
}

impl Installer {
    pub(crate) fn new(mode: InstallMode, verbose: bool) -> Result<Self> {
        Ok(Self {
            ui: Ui::new(verbose),
            mode,
            install_dir: env_path("ARGUS_INSTALL_DIR", DEFAULT_INSTALL_DIR),
            config_dir: env_path("ARGUS_CONFIG_DIR", DEFAULT_CONFIG_DIR),
            state_dir: env_path("ARGUS_STATE_DIR", DEFAULT_STATE_DIR),
            log_dir: env_path("ARGUS_LOG_DIR", DEFAULT_LOG_DIR),
            docker_config: temp_dir("argus-installer-docker")?,
        })
    }
    pub(crate) fn env_file(&self) -> PathBuf {
        self.install_dir.join(".env")
    }
    pub(crate) fn compose_file(&self) -> PathBuf {
        self.install_dir.join("compose.yaml")
    }
    pub(crate) fn caddy_file(&self) -> PathBuf {
        self.install_dir.join("Caddyfile")
    }
}

impl Drop for Installer {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.docker_config);
    }
}

pub(crate) fn parse_os_release() -> Result<BTreeMap<String, String>> {
    let path = Path::new("/etc/os-release");
    if !path.is_file() {
        bail!("/etc/os-release is required");
    }
    let mut values = BTreeMap::new();
    for line in fs::read_to_string(path)?.lines() {
        if let Some((key, value)) = line.split_once('=') {
            values.insert(
                key.to_string(),
                value.trim_matches(|c| c == '\'' || c == '"').to_string(),
            );
        }
    }
    Ok(values)
}

pub(crate) fn validate_domain(value: &str) -> Result<()> {
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

pub(crate) fn validate_basic_user(value: &str) -> Result<()> {
    if value.is_empty()
        || value
            .bytes()
            .any(|b| !(b.is_ascii_alphanumeric() || b"._-".contains(&b)))
    {
        bail!("ARGUS_BASIC_AUTH_USER may only contain letters, digits, dot, underscore and hyphen");
    }
    Ok(())
}

pub(crate) fn is_revision(value: &str) -> bool {
    value.len() == 40
        && value
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
}

pub(crate) fn new_secret(length: usize) -> String {
    let mut value = String::new();
    while value.len() < length {
        value.push_str(&Uuid::new_v4().simple().to_string());
    }
    value.truncate(length);
    value
}

pub(crate) fn value_or_secret(
    values: &BTreeMap<String, String>,
    key: &str,
    length: usize,
) -> String {
    env::var(key)
        .ok()
        .or_else(|| values.get(key).cloned())
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| new_secret(length))
}

pub(crate) fn value_or_uuid(values: &BTreeMap<String, String>, key: &str) -> String {
    env::var(key)
        .ok()
        .or_else(|| values.get(key).cloned())
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| Uuid::new_v4().to_string())
}
