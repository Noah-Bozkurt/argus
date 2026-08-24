use anyhow::{Result, bail};
use cli::lifecycle::{
    self, DEFAULT_CONFIG_DIR, DEFAULT_INSTALL_DIR, DEFAULT_LOG_DIR, DEFAULT_STATE_DIR, env_path,
    prompt_line,
};
use indicatif::{MultiProgress, ProgressBar, ProgressDrawTarget, ProgressStyle};
use secrecy::SecretString;
use std::{
    collections::BTreeMap,
    env, fs,
    io::{IsTerminal, Write},
    os::unix::fs::{OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::Duration,
};
use uuid::Uuid;

fn spinner_style() -> ProgressStyle {
    ProgressStyle::with_template("  {spinner:.cyan} {msg} · {elapsed_precise}")
        .expect("valid Argus spinner template")
        .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"])
}

fn pull_style() -> ProgressStyle {
    ProgressStyle::with_template("  [{bar:24.cyan}] {msg} · {percent:>3}% · {elapsed_precise}")
        .expect("valid Argus progress template")
        .progress_chars("█░")
}

#[derive(Clone)]
pub(crate) struct Ui {
    color: bool,
    pub(crate) verbose: bool,
    log: Arc<Mutex<Option<fs::File>>>,
}

impl Ui {
    pub(crate) fn new(verbose: bool) -> Self {
        Self {
            color: std::io::stdout().is_terminal() && env::var_os("NO_COLOR").is_none(),
            verbose,
            log: Arc::new(Mutex::new(None)),
        }
    }

    pub(crate) fn enable_log(&self, log_dir: &Path) -> Result<()> {
        fs::create_dir_all(log_dir)?;
        fs::set_permissions(log_dir, fs::Permissions::from_mode(0o700))?;
        let path = log_dir.join("installer.log");
        if path
            .metadata()
            .is_ok_and(|metadata| metadata.len() >= 10 * 1024 * 1024)
        {
            for index in (1..5).rev() {
                let source = if index == 1 {
                    path.clone()
                } else {
                    log_dir.join(format!("installer.log.{}", index - 1))
                };
                let target = log_dir.join(format!("installer.log.{index}"));
                if source.exists() {
                    let _ = fs::rename(source, target);
                }
            }
        }
        let file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .mode(0o600)
            .open(&path)?;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600))?;
        *self.log.lock().expect("installer log lock") = Some(file);
        self.record("--- Argus installer run started ---");
        Ok(())
    }

    fn redact(message: &str) -> String {
        message
            .lines()
            .map(|line| {
                let lower = line.to_ascii_lowercase();
                if [
                    "password",
                    "token",
                    "secret",
                    "authorization",
                    "database_url",
                ]
                .iter()
                .any(|word| lower.contains(word))
                {
                    "[redacted sensitive diagnostic]"
                } else {
                    line
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    pub(crate) fn record(&self, message: &str) {
        if let Ok(mut guard) = self.log.lock()
            && let Some(file) = guard.as_mut()
        {
            let _ = writeln!(file, "{}", Self::redact(message));
            let _ = file.flush();
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
        self.record("Argus installer");
        println!("{}\n", self.paint("1;36", "Argus installer"));
    }

    pub(crate) fn detail(&self, message: &str) {
        self.record(&format!("DETAIL: {message}"));
        println!("{}", self.paint("2", &format!("    {message}")));
    }

    pub(crate) fn warning(&self, message: &str) {
        self.record(&format!("WARNING: {message}"));
        eprintln!("{} {message}", self.paint("33", "  !"));
    }

    pub(crate) fn success_title(&self, message: &str) {
        println!("{}", self.paint("1;32", message));
    }

    pub(crate) fn pull_images(&self, images: &[String]) -> Result<()> {
        let message = "Downloading control-plane images";
        self.record(&format!("START: {message}"));
        let interactive = !self.verbose && std::io::stdout().is_terminal();
        if !interactive {
            println!("{} {message}", self.paint("36", "  ›"));
        }

        let multi = MultiProgress::with_draw_target(ProgressDrawTarget::stdout());
        let mut bars = BTreeMap::<String, ProgressBar>::new();
        let result = lifecycle::docker_pull_images(images, |progress| {
            if !interactive {
                return;
            }
            let image_key = progress.image.clone();
            let bar = bars.entry(image_key).or_insert_with(|| {
                let bar = ProgressBar::new_spinner();
                bar.set_style(spinner_style());
                bar.enable_steady_tick(Duration::from_millis(100));
                multi.add(bar)
            });
            let image = cli::progress::short_image_name(&progress.image);
            if progress.total > 0 {
                bar.set_length(progress.total);
                bar.set_position(progress.current.min(progress.total));
                bar.set_style(pull_style());
            }
            bar.set_message(image.clone());
            if progress.status.eq_ignore_ascii_case("complete") {
                if progress.total > 0 {
                    bar.set_position(progress.total);
                }
                bar.finish_with_message(format!("{image} · ✓"));
            }
        });

        if result.is_err() {
            let _ = multi.clear();
        }
        if result.is_ok() {
            println!("{} {message}", self.paint("32", "  ✓"));
            self.record(&format!("OK: {message}"));
        } else if let Err(error) = &result {
            self.record(&format!("FAILED: {message}: {error:#}"));
        }
        result
    }

    pub(crate) fn working<T>(&self, message: &str, work: impl FnOnce() -> Result<T>) -> Result<T> {
        self.record(&format!("START: {message}"));
        if self.verbose || !std::io::stdout().is_terminal() {
            println!("{} {message}", self.paint("36", "  ›"));
            let result = work();
            if result.is_ok() {
                println!("{} {message}", self.paint("32", "  ✓"));
                self.record(&format!("OK: {message}"));
            } else if let Err(error) = &result {
                self.record(&format!("FAILED: {message}: {error:#}"));
            }
            return result;
        }

        let bar = ProgressBar::new_spinner();
        bar.set_draw_target(ProgressDrawTarget::stdout());
        bar.set_style(spinner_style());
        bar.set_message(message.to_string());
        bar.enable_steady_tick(Duration::from_millis(100));
        let result = work();
        bar.finish_and_clear();
        if result.is_ok() {
            println!("{} {message}", self.paint("32", "  ✓"));
            self.record(&format!("OK: {message}"));
        } else if let Err(error) = &result {
            self.record(&format!("FAILED: {message}: {error:#}"));
        }
        result
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum InstallMode {
    ControlPlane,
    Agent,
    Repair,
    Update,
    Uninstall,
}

impl InstallMode {
    pub(crate) fn parse(value: &str) -> Result<Self> {
        match value {
            "control-plane" => Ok(Self::ControlPlane),
            "agent" => Ok(Self::Agent),
            "repair" => Ok(Self::Repair),
            "update" => Ok(Self::Update),
            "uninstall" => Ok(Self::Uninstall),
            _ => bail!(
                "ARGUS_INSTALL_MODE must be control-plane, agent, repair, update, or uninstall"
            ),
        }
    }

    pub(crate) fn lifecycle_name(self) -> &'static str {
        match self {
            Self::ControlPlane => "install-control-plane",
            Self::Agent => "install-managed-server",
            Self::Repair => "repair",
            Self::Update => "update",
            Self::Uninstall => "uninstall",
        }
    }
}

pub(crate) fn select_mode(requested: Option<String>) -> Result<InstallMode> {
    if let Some(value) = requested {
        return InstallMode::parse(&value);
    }
    if !lifecycle::interactive_available() {
        bail!("ARGUS_INSTALL_MODE is required in non-interactive mode");
    }
    let install_dir = env_path("ARGUS_INSTALL_DIR", DEFAULT_INSTALL_DIR);
    let state_dir = env_path("ARGUS_STATE_DIR", DEFAULT_STATE_DIR);
    if install_dir.join(".env").is_file()
        || Path::new("/usr/local/bin/argus-agent").is_file()
        || state_dir.join("agent.json").is_file()
        || state_dir.join("uninstall-recovery").is_dir()
    {
        println!("Existing Argus installation detected.\n");
        println!("  1. Repair this installation.");
        println!("  2. Update Argus.");
        println!("  3. Uninstall Argus.");
        println!("  4. Cancel.\n");
        return match prompt_line("Choose [1-4]: ")?.as_str() {
            "1" => Ok(InstallMode::Repair),
            "2" => Ok(InstallMode::Update),
            "3" => Ok(InstallMode::Uninstall),
            "4" => bail!("operation cancelled"),
            _ => bail!("invalid choice"),
        };
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
pub(crate) enum TlsMode {
    PublicAcme,
    CloudflareOrigin,
}

#[derive(Debug, Clone)]
pub(crate) struct ControlConfig {
    pub(crate) registry: String,
    pub(crate) version: String,
    pub(crate) domain: String,
    pub(crate) content_domain: String,
    pub(crate) basic_auth_user: String,
    pub(crate) basic_auth_password: SecretString,
    pub(crate) postgres_password: SecretString,
    pub(crate) web_api_token: SecretString,
    pub(crate) worker_token: SecretString,
    pub(crate) content_sync_token: SecretString,
    pub(crate) payload_secret: SecretString,
    pub(crate) org_id: String,
    pub(crate) user_id: String,
    pub(crate) bootstrap_project_id: String,
    pub(crate) bootstrap_environment_id: String,
    pub(crate) server_id: String,
    pub(crate) github_token: SecretString,
    pub(crate) rust_log: String,
    pub(crate) operator_email: String,
    pub(crate) acme_email: String,
    pub(crate) tls_mode: TlsMode,
    pub(crate) cloudflare_api_token: SecretString,
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

pub(crate) fn new_secret(length: usize) -> SecretString {
    let mut value = String::new();
    while value.len() < length {
        value.push_str(&Uuid::new_v4().simple().to_string());
    }
    value.truncate(length);
    SecretString::from(value)
}

pub(crate) fn value_or_secret(
    values: &BTreeMap<String, String>,
    key: &str,
    length: usize,
) -> SecretString {
    SecretString::from(
        env::var(key)
            .ok()
            .or_else(|| values.get(key).cloned())
            .filter(|v| !v.is_empty())
            .unwrap_or_else(|| {
                let mut value = String::new();
                while value.len() < length {
                    value.push_str(&Uuid::new_v4().simple().to_string());
                }
                value.truncate(length);
                value
            }),
    )
}

pub(crate) fn value_or_uuid(values: &BTreeMap<String, String>, key: &str) -> String {
    env::var(key)
        .ok()
        .or_else(|| values.get(key).cloned())
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| Uuid::new_v4().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use secrecy::ExposeSecret;

    #[test]
    fn installer_log_redacts_sensitive_lines() {
        let message = "service failed\nARGUS_WORKER_TOKEN=secret-value\nretry later";
        let redacted = Ui::redact(message);
        assert!(redacted.contains("service failed"));
        assert!(redacted.contains("retry later"));
        assert!(!redacted.contains("secret-value"));
    }

    #[test]
    fn generated_secrets_are_redacted_by_debug() {
        let secret = new_secret(32);
        let raw = secret.expose_secret().to_string();
        assert_eq!(raw.len(), 32);
        assert!(!format!("{secret:?}").contains(&raw));
    }

    #[test]
    fn repair_is_an_explicit_install_mode() {
        assert_eq!(InstallMode::parse("repair").unwrap(), InstallMode::Repair);
    }
}
