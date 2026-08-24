use super::{
    installer_shared::{ControlConfig, TlsMode, new_secret, validate_basic_user, validate_domain},
    installer_ui::{MenuChoice, MenuItem, menu_select},
};
use anyhow::{Result, bail};
use cli::lifecycle::{self, prompt_line, prompt_secret};
use secrecy::{ExposeSecret, SecretString};

const INSTALL: usize = 5;
const CANCEL: usize = 6;

fn rows(config: &ControlConfig) -> Vec<String> {
    vec![
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

fn choose(config: &ControlConfig) -> Result<usize> {
    let items = rows(config)
        .into_iter()
        .map(MenuItem::new)
        .collect::<Vec<_>>();
    match menu_select(None, "Review installation", &items)? {
        MenuChoice::Selected(index) => Ok(index),
        MenuChoice::Cancelled => Ok(CANCEL),
    }
}

fn password_is_valid(value: &str) -> bool {
    value.len() >= 12
}

pub(crate) fn review_control_install(config: &mut ControlConfig) -> Result<()> {
    if config.existing_install || !lifecycle::interactive_available() {
        return Ok(());
    }

    loop {
        match choose(config)? {
            0 => {
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
            1 => {
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
            2 => {
                let value = prompt_line("Certificate email: ")?;
                if !value.contains('@') || value.starts_with('@') || value.ends_with('@') {
                    eprintln!("Enter a valid email address.");
                    let _ = prompt_line("Press Enter to continue.");
                } else {
                    config.acme_email = value;
                }
            }
            3 => {
                let value = prompt_line("Login username: ")?;
                if let Err(error) = validate_basic_user(&value) {
                    eprintln!("{error}");
                    let _ = prompt_line("Press Enter to continue.");
                } else {
                    config.basic_auth_user = value;
                }
            }
            4 => {
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
                        config.basic_auth_password = SecretString::from(first);
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
        let config = ControlConfig {
            registry: "ghcr.io/example".to_string(),
            version: "main".to_string(),
            domain: "app.example.com".to_string(),
            content_domain: "content.example.com".to_string(),
            basic_auth_user: "argus".to_string(),
            basic_auth_password: SecretString::from("never-print-this-password".to_string()),
            postgres_password: SecretString::from(String::new()),
            web_api_token: SecretString::from(String::new()),
            worker_token: SecretString::from(String::new()),
            content_sync_token: SecretString::from(String::new()),
            payload_secret: SecretString::from(String::new()),
            org_id: String::new(),
            user_id: String::new(),
            bootstrap_project_id: String::new(),
            bootstrap_environment_id: String::new(),
            server_id: String::new(),
            github_token: SecretString::from(String::new()),
            rust_log: "info".to_string(),
            operator_email: "operator@argus.local".to_string(),
            acme_email: "admin@example.com".to_string(),
            tls_mode: TlsMode::PublicAcme,
            cloudflare_api_token: SecretString::from(String::new()),
            org_name: "Argus".to_string(),
            generated_basic_password: false,
            existing_install: false,
        };
        let rendered = rows(&config).join("\n");
        assert!(!rendered.contains(config.basic_auth_password.expose_secret()));
    }

    #[test]
    fn login_password_has_a_minimum_length() {
        assert!(!password_is_valid("short"));
        assert!(password_is_valid("twelve-chars"));
    }
}
