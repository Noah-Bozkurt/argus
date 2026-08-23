use anyhow::{Result, bail};
use cli::lifecycle::{
    self, DEFAULT_CONFIG_DIR, DEFAULT_INSTALL_DIR, DEFAULT_LOG_DIR, DEFAULT_STATE_DIR, env_path,
    prompt_line,
};
use std::{
    collections::BTreeMap,
    env, fs,
    io::{IsTerminal, Write},
    os::unix::fs::{OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant},
};
use uuid::Uuid;

const PROGRESS_WIDTH: usize = 24;
const PROGRESS_PULSE: usize = 6;

fn indeterminate_bar(frame: usize) -> String {
    let travel = PROGRESS_WIDTH - PROGRESS_PULSE;
    let cycle = travel * 2;
    let position = if cycle == 0 {
        0
    } else {
        let phase = frame % cycle;
        if phase <= travel {
            phase
        } else {
            cycle - phase
        }
    };
    let mut cells = vec!['░'; PROGRESS_WIDTH];
    for cell in cells.iter_mut().skip(position).take(PROGRESS_PULSE) {
        *cell = '█';
    }
    cells.into_iter().collect()
}

fn determinate_bar(current: u64, total: u64) -> String {
    if total == 0 {
        return indeterminate_bar(0);
    }
    let filled = ((current.min(total) as u128 * PROGRESS_WIDTH as u128) / total as u128) as usize;
    let mut cells = vec!['░'; PROGRESS_WIDTH];
    for cell in cells.iter_mut().take(filled) {
        *cell = '█';
    }
    cells.into_iter().collect()
}

fn elapsed_label(seconds: u64) -> String {
    if seconds >= 60 {
        format!("{}m {:02}s", seconds / 60, seconds % 60)
    } else {
        format!("{seconds}s")
    }
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
        let started = Instant::now();
        if interactive {
            let bar = self.paint("36", &format!("[{}]", indeterminate_bar(0)));
            print!("\r\x1b[2K  {bar} {message} · connecting");
            let _ = std::io::stdout().flush();
        } else {
            println!("{} {message}", self.paint("36", "  ›"));
        }

        let result = lifecycle::docker_pull_images(images, |progress| {
            if !interactive {
                return;
            }
            let (bar, percentage) = if progress.total > 0 {
                let percent = (progress.current.min(progress.total) as u128 * 100
                    / progress.total as u128) as u64;
                (
                    determinate_bar(progress.current, progress.total),
                    format!(" · {percent:>3}%"),
                )
            } else {
                (indeterminate_bar(0), String::new())
            };
            let bar = self.paint("36", &format!("[{bar}]"));
            let image = lifecycle::docker_short_image_name(&progress.image);
            let status = progress.status.to_ascii_lowercase();
            print!(
                "\r\x1b[2K  {bar} {message} · {image} · {status}{percentage} · {}",
                elapsed_label(started.elapsed().as_secs())
            );
            let _ = std::io::stdout().flush();
        });

        if interactive {
            print!("\r\x1b[2K");
            let _ = std::io::stdout().flush();
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
        let running = Arc::new(AtomicBool::new(true));
        let flag = Arc::clone(&running);
        let message_owned = message.to_string();
        let color = self.color;
        let progress = thread::spawn(move || {
            let mut frame = 0usize;
            let started = std::time::Instant::now();
            while flag.load(Ordering::Relaxed) {
                let elapsed = started.elapsed().as_secs();
                let bar = indeterminate_bar(frame);
                let bar = if color {
                    format!("\x1b[36m[{bar}]\x1b[0m")
                } else {
                    format!("[{bar}]")
                };
                let activity = if elapsed >= 15 {
                    " · still working"
                } else {
                    ""
                };
                print!(
                    "\r\x1b[2K  {bar} {message_owned} · {}{activity}",
                    elapsed_label(elapsed)
                );
                let _ = std::io::stdout().flush();
                frame = frame.wrapping_add(1);
                thread::sleep(Duration::from_millis(100));
            }
        });
        let result = work();
        running.store(false, Ordering::Relaxed);
        let _ = progress.join();
        print!("\r\x1b[2K");
        let _ = std::io::stdout().flush();
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
    pub(crate) acme_email: String,
    pub(crate) tls_mode: TlsMode,
    pub(crate) cloudflare_api_token: String,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn installer_log_redacts_sensitive_lines() {
        let message = "service failed\nARGUS_WORKER_TOKEN=secret-value\nretry later";
        let redacted = Ui::redact(message);
        assert!(redacted.contains("service failed"));
        assert!(redacted.contains("retry later"));
        assert!(!redacted.contains("secret-value"));
    }

    #[test]
    fn installer_progress_bar_is_fixed_width_and_moves() {
        let first = indeterminate_bar(0);
        let later = indeterminate_bar(7);
        assert_eq!(first.chars().count(), PROGRESS_WIDTH);
        assert_eq!(later.chars().count(), PROGRESS_WIDTH);
        assert_ne!(first, later);
        assert_eq!(
            first.chars().filter(|cell| *cell == '█').count(),
            PROGRESS_PULSE
        );
    }

    #[test]
    fn installer_elapsed_time_becomes_compact_minutes() {
        assert_eq!(elapsed_label(8), "8s");
        assert_eq!(elapsed_label(65), "1m 05s");
    }

    #[test]
    fn repair_is_an_explicit_install_mode() {
        assert_eq!(InstallMode::parse("repair").unwrap(), InstallMode::Repair);
    }
}
