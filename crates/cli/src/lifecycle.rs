use anyhow::{Context, Result, bail};
use std::{
    collections::BTreeMap,
    env,
    fs::{self, File, OpenOptions},
    io::{self, BufRead, BufReader, IsTerminal, Read, Write},
    os::unix::fs::{OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
};
use uuid::Uuid;

pub const DEFAULT_INSTALL_DIR: &str = "/opt/argus";
pub const DEFAULT_CONFIG_DIR: &str = "/etc/argus";
pub const DEFAULT_STATE_DIR: &str = "/var/lib/argus";
pub const DEFAULT_LOG_DIR: &str = "/var/log/argus";
pub const DEFAULT_REGISTRY: &str = "ghcr.io/noah-bozkurt";

#[derive(Debug, Clone)]
pub struct RegistryCredentials {
    pub registry: String,
    pub username: String,
    pub token: String,
}

#[derive(Debug, Clone)]
pub struct UninstallOptions {
    pub yes: bool,
    pub purge_data: bool,
    pub install_dir: PathBuf,
    pub config_dir: PathBuf,
    pub state_dir: PathBuf,
    pub log_dir: PathBuf,
}

impl UninstallOptions {
    pub fn from_env(yes: bool, purge_data: bool) -> Self {
        Self {
            yes,
            purge_data,
            install_dir: env_path("ARGUS_INSTALL_DIR", DEFAULT_INSTALL_DIR),
            config_dir: env_path("ARGUS_CONFIG_DIR", DEFAULT_CONFIG_DIR),
            state_dir: env_path("ARGUS_STATE_DIR", DEFAULT_STATE_DIR),
            log_dir: env_path("ARGUS_LOG_DIR", DEFAULT_LOG_DIR),
        }
    }
}

pub fn env_path(name: &str, default: &str) -> PathBuf {
    env::var_os(name)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(default))
}

pub fn require_root() -> Result<()> {
    let output = Command::new("id").arg("-u").output().context("run id -u")?;
    if !output.status.success() || String::from_utf8_lossy(&output.stdout).trim() != "0" {
        bail!("run as root (sudo ...)");
    }
    Ok(())
}

pub fn command_exists(name: &str) -> bool {
    Command::new("sh")
        .args(["-c", "command -v \"$1\" >/dev/null 2>&1", "sh", name])
        .status()
        .is_ok_and(|status| status.success())
}

pub fn run(program: &str, args: &[&str]) -> Result<()> {
    let status = Command::new(program)
        .args(args)
        .status()
        .with_context(|| format!("run {program}"))?;
    if !status.success() {
        bail!("{program} failed with {status}");
    }
    Ok(())
}

pub fn run_quiet(program: &str, args: &[&str]) -> Result<()> {
    let output = Command::new(program)
        .args(args)
        .output()
        .with_context(|| format!("run {program}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        if stderr.is_empty() {
            bail!("{program} failed with {}", output.status);
        }
        bail!("{program} failed: {stderr}");
    }
    Ok(())
}

pub fn output(program: &str, args: &[&str]) -> Result<String> {
    let output = output_raw(program, args)?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        if stderr.is_empty() {
            bail!("{program} failed with {}", output.status);
        }
        bail!("{program} failed: {stderr}");
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

pub fn output_raw(program: &str, args: &[&str]) -> Result<Output> {
    Command::new(program)
        .args(args)
        .output()
        .with_context(|| format!("run {program}"))
}

pub fn run_with_input(program: &str, args: &[&str], input: &[u8]) -> Result<()> {
    let mut child = Command::new(program)
        .args(args)
        .stdin(Stdio::piped())
        .spawn()
        .with_context(|| format!("run {program}"))?;
    child
        .stdin
        .as_mut()
        .context("open child stdin")?
        .write_all(input)
        .with_context(|| format!("write input to {program}"))?;
    let status = child
        .wait()
        .with_context(|| format!("wait for {program}"))?;
    if !status.success() {
        bail!("{program} failed with {status}");
    }
    Ok(())
}

pub fn run_with_input_env(
    program: &str,
    args: &[&str],
    input: &[u8],
    envs: &[(&str, &str)],
) -> Result<()> {
    let mut command = Command::new(program);
    command.args(args).stdin(Stdio::piped());
    for (key, value) in envs {
        command.env(key, value);
    }
    let mut child = command.spawn().with_context(|| format!("run {program}"))?;
    child
        .stdin
        .as_mut()
        .context("open child stdin")?
        .write_all(input)
        .with_context(|| format!("write input to {program}"))?;
    let status = child
        .wait()
        .with_context(|| format!("wait for {program}"))?;
    if !status.success() {
        bail!("{program} failed with {status}");
    }
    Ok(())
}

fn open_tty() -> Option<File> {
    OpenOptions::new()
        .read(true)
        .write(true)
        .open("/dev/tty")
        .ok()
}

pub fn interactive_available() -> bool {
    io::stdin().is_terminal() || open_tty().is_some()
}

pub fn prompt_line(prompt: &str) -> Result<String> {
    if let Some(mut tty) = open_tty() {
        write!(tty, "{prompt}")?;
        tty.flush()?;
        let mut value = String::new();
        BufReader::new(tty.try_clone()?).read_line(&mut value)?;
        return Ok(value.trim().to_string());
    }
    if !io::stdin().is_terminal() {
        bail!("interactive input is unavailable");
    }
    print!("{prompt}");
    io::stdout().flush()?;
    let mut value = String::new();
    io::stdin().read_line(&mut value)?;
    Ok(value.trim().to_string())
}

struct EchoGuard {
    tty: Option<File>,
}

impl EchoGuard {
    fn set(tty: Option<&File>, enabled: bool) -> Result<()> {
        let mut command = Command::new("stty");
        command.arg(if enabled { "echo" } else { "-echo" });
        if let Some(tty) = tty {
            command.stdin(Stdio::from(tty.try_clone()?));
        }
        let status = command.status().context("change terminal echo")?;
        if !status.success() {
            bail!("could not change terminal echo");
        }
        Ok(())
    }
}

impl Drop for EchoGuard {
    fn drop(&mut self) {
        let _ = Self::set(self.tty.as_ref(), true);
    }
}

pub fn prompt_secret(prompt: &str) -> Result<String> {
    if let Some(mut tty) = open_tty() {
        write!(tty, "{prompt}")?;
        tty.flush()?;
        EchoGuard::set(Some(&tty), false)?;
        let guard = EchoGuard {
            tty: Some(tty.try_clone()?),
        };
        let mut value = String::new();
        let read = BufReader::new(tty.try_clone()?).read_line(&mut value);
        drop(guard);
        writeln!(tty)?;
        read?;
        return Ok(value.trim().to_string());
    }
    if !io::stdin().is_terminal() {
        bail!("interactive input is unavailable");
    }
    print!("{prompt}");
    io::stdout().flush()?;
    EchoGuard::set(None, false)?;
    let guard = EchoGuard { tty: None };
    let mut value = String::new();
    let read = io::stdin().read_line(&mut value);
    drop(guard);
    println!();
    read?;
    Ok(value.trim().to_string())
}

pub fn valid_github_username(value: &str) -> bool {
    if value.is_empty() || value.len() > 39 || value.contains("--") {
        return false;
    }
    let bytes = value.as_bytes();
    let valid_char = |b: u8| b.is_ascii_alphanumeric() || b == b'-';
    bytes.iter().all(|b| valid_char(*b))
        && bytes.first().is_some_and(|b| b.is_ascii_alphanumeric())
        && bytes.last().is_some_and(|b| b.is_ascii_alphanumeric())
}

pub fn load_registry_credentials(config_dir: &Path) -> Result<Option<RegistryCredentials>> {
    let path = config_dir.join("registry.env");
    if !path.exists() {
        return Ok(None);
    }
    let mode = fs::metadata(&path)?.permissions().mode() & 0o777;
    if mode != 0o600 {
        bail!("{} must have mode 0600", path.display());
    }
    let values = read_env_file(&path)?;
    let Some(username) = values.get("ARGUS_REGISTRY_USERNAME").cloned() else {
        return Ok(None);
    };
    let Some(token) = values.get("ARGUS_REGISTRY_TOKEN").cloned() else {
        return Ok(None);
    };
    Ok(Some(RegistryCredentials {
        registry: values
            .get("ARGUS_REGISTRY")
            .cloned()
            .unwrap_or_else(|| DEFAULT_REGISTRY.to_string()),
        username,
        token,
    }))
}

pub fn collect_registry_credentials(
    config_dir: &Path,
    username_override: Option<&str>,
) -> Result<RegistryCredentials> {
    let stored = load_registry_credentials(config_dir)?;
    let registry = env::var("ARGUS_REGISTRY")
        .ok()
        .or_else(|| stored.as_ref().map(|item| item.registry.clone()))
        .unwrap_or_else(|| DEFAULT_REGISTRY.to_string());
    let username = match username_override
        .map(ToOwned::to_owned)
        .or_else(|| env::var("ARGUS_REGISTRY_USERNAME_OVERRIDE").ok())
        .or_else(|| env::var("ARGUS_REGISTRY_USERNAME").ok())
        .or_else(|| stored.as_ref().map(|item| item.username.clone()))
    {
        Some(value) => value,
        None => prompt_line("GitHub username: ")?,
    };
    if !valid_github_username(&username) {
        bail!("invalid GitHub username");
    }
    let token = match env::var("ARGUS_REGISTRY_TOKEN")
        .ok()
        .or_else(|| stored.as_ref().map(|item| item.token.clone()))
    {
        Some(value) => value,
        None => prompt_secret("GitHub token (classic PAT with read:packages): ")?,
    };
    if token.is_empty() {
        bail!("GitHub token is required");
    }
    Ok(RegistryCredentials {
        registry,
        username,
        token,
    })
}

pub fn docker_login(credentials: &RegistryCredentials, docker_config: &Path) -> Result<()> {
    fs::create_dir_all(docker_config)?;
    fs::set_permissions(docker_config, fs::Permissions::from_mode(0o700))?;
    let registry_host = credentials
        .registry
        .split('/')
        .next()
        .unwrap_or(&credentials.registry);
    let mut child = Command::new("docker")
        .args([
            "login",
            registry_host,
            "-u",
            &credentials.username,
            "--password-stdin",
        ])
        .env("DOCKER_CONFIG", docker_config)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .spawn()
        .context("start docker login")?;
    child
        .stdin
        .as_mut()
        .context("open docker login stdin")?
        .write_all(credentials.token.as_bytes())?;
    let status = child.wait().context("wait for docker login")?;
    if !status.success() {
        bail!("GHCR login failed; verify the classic PAT has read:packages");
    }
    Ok(())
}

pub fn save_registry_credentials(
    config_dir: &Path,
    credentials: &RegistryCredentials,
) -> Result<()> {
    let existed = config_dir.exists();
    fs::create_dir_all(config_dir)?;
    if !existed {
        fs::set_permissions(config_dir, fs::Permissions::from_mode(0o750))?;
    }
    let values = [
        ("ARGUS_REGISTRY", credentials.registry.as_str()),
        ("ARGUS_REGISTRY_USERNAME", credentials.username.as_str()),
        ("ARGUS_REGISTRY_TOKEN", credentials.token.as_str()),
    ];
    write_env_file(&config_dir.join("registry.env"), &values, 0o600)
}

pub fn registry_login(username_override: Option<&str>) -> Result<()> {
    require_root().context("registry login must run as root")?;
    if !command_exists("docker") {
        bail!("docker is required");
    }
    let config_dir = env_path("ARGUS_CONFIG_DIR", DEFAULT_CONFIG_DIR);
    let credentials = collect_registry_credentials(&config_dir, username_override)?;
    let docker_config = temp_dir("argus-registry")?;
    let result = docker_login(&credentials, &docker_config)
        .and_then(|_| save_registry_credentials(&config_dir, &credentials));
    let _ = fs::remove_dir_all(&docker_config);
    result?;
    println!(
        "Stored validated GHCR credentials in {} (mode 0600).",
        config_dir.join("registry.env").display()
    );
    Ok(())
}

pub fn read_env_file(path: &Path) -> Result<BTreeMap<String, String>> {
    let mut data = String::new();
    File::open(path)
        .with_context(|| format!("open {}", path.display()))?
        .read_to_string(&mut data)?;
    let mut result = BTreeMap::new();
    for raw_line in data.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        result.insert(key.trim().to_string(), unquote_env(value.trim()));
    }
    Ok(result)
}

fn unquote_env(value: &str) -> String {
    if value.len() >= 2 {
        if let Some(inner) = value.strip_prefix('\'').and_then(|v| v.strip_suffix('\'')) {
            return inner.replace("'\\''", "'");
        }
        if let Some(inner) = value.strip_prefix('"').and_then(|v| v.strip_suffix('"')) {
            return inner.to_string();
        }
    }
    value.to_string()
}

pub fn write_env_file(path: &Path, values: &[(&str, &str)], mode: u32) -> Result<()> {
    let parent = path.parent().context("environment file has no parent")?;
    fs::create_dir_all(parent)?;
    let tmp = parent.join(format!(
        ".{}.tmp-{}-{}",
        path.file_name().and_then(|v| v.to_str()).unwrap_or("env"),
        std::process::id(),
        Uuid::new_v4().simple()
    ));
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .mode(mode)
        .open(&tmp)
        .with_context(|| format!("create {}", tmp.display()))?;
    for (key, value) in values {
        writeln!(file, "{key}={}", quote_env(value))?;
    }
    file.sync_all()?;
    fs::set_permissions(&tmp, fs::Permissions::from_mode(mode))?;
    fs::rename(&tmp, path).with_context(|| format!("replace {}", path.display()))?;
    Ok(())
}

pub fn quote_env(value: &str) -> String {
    if value
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || b"._:/@+-".contains(&b))
    {
        return value.to_string();
    }
    format!("'{}'", value.replace('\'', "'\\''"))
}

pub fn temp_dir(prefix: &str) -> Result<PathBuf> {
    let path = env::temp_dir().join(format!(
        "{prefix}-{}-{}",
        std::process::id(),
        Uuid::new_v4().simple()
    ));
    fs::create_dir(&path)?;
    fs::set_permissions(&path, fs::Permissions::from_mode(0o700))?;
    Ok(path)
}

pub fn copy_file(source: &Path, target: &Path, mode: u32) -> Result<()> {
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::copy(source, target)
        .with_context(|| format!("copy {} to {}", source.display(), target.display()))?;
    fs::set_permissions(target, fs::Permissions::from_mode(mode))?;
    Ok(())
}

pub fn remove_path(path: &Path) -> Result<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_dir() => fs::remove_dir_all(path)?,
        Ok(_) => fs::remove_file(path)?,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    Ok(())
}

fn preserve_recovery_files(options: &UninstallOptions) -> Result<PathBuf> {
    let recovery_dir = options.state_dir.join("uninstall-recovery");
    fs::create_dir_all(&recovery_dir)?;
    fs::set_permissions(&recovery_dir, fs::Permissions::from_mode(0o700))?;
    for (source, name) in [
        (options.install_dir.join(".env"), "runtime.env"),
        (options.install_dir.join("compose.yaml"), "compose.yaml"),
        (options.install_dir.join("Caddyfile"), "Caddyfile"),
        (options.config_dir.join("registry.env"), "registry.env"),
        (options.config_dir.join("agent.env"), "agent.env"),
        (options.config_dir.join("helper.env"), "helper.env"),
        (options.config_dir.join("revision"), "revision"),
    ] {
        if source.is_file() {
            copy_file(&source, &recovery_dir.join(name), 0o600)?;
        }
    }
    Ok(recovery_dir)
}

pub fn uninstall(options: UninstallOptions) -> Result<()> {
    require_root().context("uninstall must run as root")?;
    if !options.yes {
        if !interactive_available() {
            bail!("confirmation required; rerun with --yes");
        }
        println!("This will stop Argus and remove its binaries and configuration.");
        if options.purge_data {
            println!();
            println!(
                "WARNING: Docker volumes, state, backups, and logs will be permanently deleted."
            );
            println!("This cannot be undone without an external backup.");
        }
        let answer = prompt_line("Type UNINSTALL ARGUS to continue: ")?;
        if answer != "UNINSTALL ARGUS" {
            bail!("uninstall cancelled");
        }
    }

    println!("[argus-uninstall] stopping Argus services");
    let _ = Command::new("systemctl")
        .args([
            "disable",
            "--now",
            "argus-agent.service",
            "argus-helper.service",
        ])
        .status();

    let env_file = options.install_dir.join(".env");
    let compose_file = options.install_dir.join("compose.yaml");
    if env_file.is_file() && compose_file.is_file() && command_exists("docker") {
        let mut args = vec![
            "compose".to_string(),
            "--project-directory".to_string(),
            options.install_dir.display().to_string(),
            "--env-file".to_string(),
            env_file.display().to_string(),
            "-f".to_string(),
            compose_file.display().to_string(),
            "down".to_string(),
            "--remove-orphans".to_string(),
        ];
        if options.purge_data {
            args.push("--volumes".to_string());
        }
        let refs = args.iter().map(String::as_str).collect::<Vec<_>>();
        run("docker", &refs).context("stop the Argus control-plane stack")?;
    }

    let recovery_dir = if options.purge_data {
        None
    } else {
        Some(preserve_recovery_files(&options).context("preserve recovery configuration")?)
    };

    println!("[argus-uninstall] removing Argus binaries and configuration");
    for path in [
        "/usr/local/bin/argus-agent",
        "/usr/local/bin/argus-helper",
        "/usr/local/bin/argusctl",
        "/usr/local/bin/argus-installer",
        "/etc/systemd/system/argus-agent.service",
        "/etc/systemd/system/argus-helper.service",
    ] {
        remove_path(Path::new(path))?;
    }
    let _ = Command::new("systemctl").arg("daemon-reload").status();
    remove_path(&options.install_dir)?;
    remove_path(&options.config_dir)?;

    if options.purge_data {
        println!("[argus-uninstall] purging Argus state, backups, and logs");
        remove_path(&options.state_dir)?;
        remove_path(&options.log_dir)?;
        let _ = Command::new("userdel")
            .arg("argus")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        let _ = Command::new("groupdel")
            .arg("argus")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        println!(
            "Argus and its data were removed. This cannot be recovered without an external backup."
        );
    } else {
        println!("Argus was removed. State and Docker volumes were preserved for recovery.");
        println!("Preserved state: {}", options.state_dir.display());
        if let Some(recovery_dir) = recovery_dir {
            println!("Recovery configuration: {}", recovery_dir.display());
            println!("Run the public Argus installer and choose Repair to restore this host.");
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn github_username_validation_matches_github_rules_used_by_argus() {
        for valid in ["octocat", "Noah-Bozkurt", "a", "a1-b2"] {
            assert!(valid_github_username(valid), "{valid}");
        }
        for invalid in ["", "-octocat", "octocat-", "octo--cat", "octo_cat", "a b"] {
            assert!(!valid_github_username(invalid), "{invalid}");
        }
    }

    #[test]
    fn env_quote_round_trip_for_credentials() {
        for value in [
            "plain",
            "ghp_token",
            "value with space",
            "quote'value",
            "dollar$value",
        ] {
            let quoted = quote_env(value);
            assert_eq!(unquote_env(&quoted), value);
        }
    }
}
