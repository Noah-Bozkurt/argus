use super::installer_shared::{
    ControlConfig, TlsMode, new_secret, validate_basic_user, validate_domain,
};
use anyhow::{Context, Result, bail};
use cli::lifecycle::{self, RegistryCredentials, prompt_line, prompt_secret};
use std::{
    fs::{File, OpenOptions},
    io::{Read, Write},
    process::{Command, Stdio},
};

const INSTALL: usize = 7;
const CANCEL: usize = 8;

struct TerminalMode {
    tty: File,
    original: String,
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
            .context("enable review navigation")?;
        if !status.success() {
            bail!("could not enable review navigation");
        }
        Ok(Self { tty, original })
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

fn rows(credentials: &RegistryCredentials, config: &ControlConfig) -> Vec<String> {
    vec![
        format!("GitHub username:  {}", credentials.username),
        "GitHub token:     •••••••••••• configured".to_string(),
        format!("Primary domain:   {}", config.domain),
        format!("Content domain:   {}", config.content_domain),
        format!("Certificate email: {}", config.acme_email),
        format!("Login username:   {}", config.basic_auth_user),
        format!(
            "Login password:   •••••••••••• {}",
            if config.generated_basic_password {
                "generated"
            } else {
                "configured"
            }
        ),
        "Install Argus".to_string(),
        "Cancel".to_string(),
    ]
}

fn select_row(credentials: &RegistryCredentials, config: &ControlConfig) -> Result<usize> {
    let mut terminal = TerminalMode::enter()?;
    let mut selected = 0usize;
    loop {
        write!(
            terminal.tty,
            "\x1b[?25l\x1b[2J\x1b[HArgus installation review\n\n"
        )?;
        for (index, row) in rows(credentials, config).iter().enumerate() {
            if index == selected {
                writeln!(terminal.tty, "\x1b[36m› {row}\x1b[0m")?;
            } else {
                writeln!(terminal.tty, "  {row}")?;
            }
        }
        write!(
            terminal.tty,
            "\n↑/↓ navigate  •  Enter edit/select  •  q cancel\n"
        )?;
        terminal.tty.flush()?;

        let mut byte = [0u8; 1];
        terminal.tty.read_exact(&mut byte)?;
        match byte[0] {
            b'\r' | b'\n' => return Ok(selected),
            b'q' | b'Q' => return Ok(CANCEL),
            0x1b => {
                let mut sequence = [0u8; 2];
                if terminal.tty.read_exact(&mut sequence).is_ok() && sequence[0] == b'[' {
                    match sequence[1] {
                        b'A' => selected = selected.checked_sub(1).unwrap_or(CANCEL),
                        b'B' => selected = (selected + 1) % (CANCEL + 1),
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
}

fn select_row_fallback(credentials: &RegistryCredentials, config: &ControlConfig) -> Result<usize> {
    println!("\nReview installation\n");
    for (index, row) in rows(credentials, config).iter().enumerate() {
        println!("  {}. {row}", index + 1);
    }
    let answer = prompt_line("\nChoose a value to edit, install, or cancel [1-9]: ")?;
    let selected = answer
        .parse::<usize>()
        .context("enter a number from 1 to 9")?;
    if !(1..=9).contains(&selected) {
        bail!("enter a number from 1 to 9");
    }
    Ok(selected - 1)
}

fn choose(credentials: &RegistryCredentials, config: &ControlConfig) -> Result<usize> {
    select_row(credentials, config).or_else(|_| select_row_fallback(credentials, config))
}

fn password_is_valid(value: &str) -> bool {
    value.len() >= 12
}

pub(crate) fn review_control_install(
    credentials: &mut RegistryCredentials,
    config: &mut ControlConfig,
) -> Result<()> {
    if config.existing_install || !lifecycle::interactive_available() {
        return Ok(());
    }

    loop {
        match choose(credentials, config)? {
            0 => {
                let value = prompt_line("GitHub username: ")?;
                if lifecycle::valid_github_username(&value) {
                    credentials.username = value;
                } else {
                    eprintln!("Invalid GitHub username. Press Enter to continue.");
                    let _ = prompt_line("");
                }
            }
            1 => {
                let value = prompt_secret("GitHub token (leave empty to keep current): ")?;
                if !value.is_empty() {
                    credentials.token = value;
                }
            }
            2 => {
                let old = config.domain.clone();
                let value = prompt_line("Primary domain: ")?.to_ascii_lowercase();
                if let Err(error) = validate_domain(&value) {
                    eprintln!("{error}");
                    let _ = prompt_line("Press Enter to continue.");
                } else if value == config.content_domain {
                    eprintln!("Web and content domains must differ.");
                    let _ = prompt_line("Press Enter to continue.");
                } else {
                    if config.content_domain == format!("content.{old}") {
                        config.content_domain = format!("content.{value}");
                    }
                    config.domain = value;
                }
            }
            3 => {
                let value = prompt_line("Content domain: ")?.to_ascii_lowercase();
                if let Err(error) = validate_domain(&value) {
                    eprintln!("{error}");
                    let _ = prompt_line("Press Enter to continue.");
                } else if value == config.domain {
                    eprintln!("Web and content domains must differ.");
                    let _ = prompt_line("Press Enter to continue.");
                } else {
                    config.content_domain = value;
                }
            }
            4 => {
                let value = prompt_line("Certificate email: ")?;
                if !value.contains('@') || value.starts_with('@') || value.ends_with('@') {
                    eprintln!("Enter a valid email address.");
                    let _ = prompt_line("Press Enter to continue.");
                } else {
                    config.acme_email = value;
                }
            }
            5 => {
                let value = prompt_line("Login username: ")?;
                if let Err(error) = validate_basic_user(&value) {
                    eprintln!("{error}");
                    let _ = prompt_line("Press Enter to continue.");
                } else {
                    config.basic_auth_user = value;
                }
            }
            6 => {
                let first = prompt_secret("Login password (Enter to generate): ")?;
                if first.is_empty() {
                    config.basic_auth_password = new_secret(24);
                    config.generated_basic_password = true;
                } else if !password_is_valid(&first) {
                    eprintln!("Password must be at least 12 characters.");
                    let _ = prompt_line("Press Enter to continue.");
                } else {
                    let second = prompt_secret("Confirm login password: ")?;
                    if first == second {
                        config.basic_auth_password = first;
                        config.generated_basic_password = false;
                    } else {
                        eprintln!("Passwords do not match.");
                        let _ = prompt_line("Press Enter to continue.");
                    }
                }
            }
            INSTALL => return Ok(()),
            CANCEL => bail!("installation cancelled"),
            _ => unreachable!(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn review_rows_never_contain_secret_values() {
        let credentials = RegistryCredentials {
            registry: "ghcr.io/example".to_string(),
            username: "octocat".to_string(),
            token: "never-print-this-token".to_string(),
        };
        let config = ControlConfig {
            registry: credentials.registry.clone(),
            version: "main".to_string(),
            domain: "app.example.com".to_string(),
            content_domain: "content.example.com".to_string(),
            basic_auth_user: "argus".to_string(),
            basic_auth_password: "never-print-this-password".to_string(),
            postgres_password: String::new(),
            web_api_token: String::new(),
            worker_token: String::new(),
            content_sync_token: String::new(),
            payload_secret: String::new(),
            org_id: String::new(),
            user_id: String::new(),
            bootstrap_project_id: String::new(),
            bootstrap_environment_id: String::new(),
            server_id: String::new(),
            github_token: String::new(),
            rust_log: "info".to_string(),
            operator_email: "operator@argus.local".to_string(),
            acme_email: "admin@example.com".to_string(),
            tls_mode: TlsMode::PublicAcme,
            cloudflare_api_token: String::new(),
            org_name: "Argus".to_string(),
            generated_basic_password: false,
            existing_install: false,
        };
        let rendered = rows(&credentials, &config).join("\n");
        assert!(!rendered.contains(&credentials.token));
        assert!(!rendered.contains(&config.basic_auth_password));
    }

    #[test]
    fn login_password_has_a_minimum_length() {
        assert!(!password_is_valid("short"));
        assert!(password_is_valid("twelve-chars"));
    }
}
