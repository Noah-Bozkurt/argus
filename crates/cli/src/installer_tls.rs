use super::installer_shared::{ControlConfig, Installer, TlsMode};
use anyhow::{Context, Result, bail};
use cli::lifecycle::{self, prompt_secret};
use reqwest::{Client, StatusCode};
use serde_json::{Value, json};
use std::{fs, os::unix::fs::PermissionsExt, time::Duration};

const ORIGIN_CA_ENDPOINT: &str = "https://api.cloudflare.com/client/v4/certificates";

fn runtime() -> Result<tokio::runtime::Runtime> {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("create TLS setup runtime")
}

async fn cloudflare_proxied(client: &Client, domain: &str) -> bool {
    let url = format!("https://{domain}/cdn-cgi/trace");
    client
        .get(url)
        .send()
        .await
        .ok()
        .filter(|response| response.status().is_success())
        .and_then(|response| {
            let has_ray = response.headers().contains_key("cf-ray");
            has_ray.then_some(response)
        })
        .is_some()
}

impl Installer {
    pub(crate) fn configure_tls(&self, config: &mut ControlConfig) -> Result<()> {
        if config.existing_install {
            return Ok(());
        }
        let client = Client::builder()
            .timeout(Duration::from_secs(8))
            .build()
            .context("build Cloudflare detection client")?;
        let (web, content) = runtime()?.block_on(async {
            tokio::join!(
                cloudflare_proxied(&client, &config.domain),
                cloudflare_proxied(&client, &config.content_domain)
            )
        });
        if !(web && content) {
            self.ui.detail(
                "Cloudflare proxy was not confirmed for both domains; using Let's Encrypt with ZeroSSL fallback",
            );
            config.tls_mode = TlsMode::PublicAcme;
            return Ok(());
        }

        self.ui.detail("Cloudflare proxy detected for both domains");
        if config.cloudflare_api_token.is_empty() && lifecycle::interactive_available() {
            println!(
                "\nCloudflare proxy detected. Argus can create a Cloudflare Origin CA certificate."
            );
            println!("The token needs Zone / SSL and Certificates / Edit permission.");
            println!(
                "Keep Cloudflare SSL/TLS mode set to Full (strict). Leaving this blank uses public ACME.\n"
            );
            config.cloudflare_api_token = prompt_secret("Cloudflare API token (optional): ")?;
        }
        if config.cloudflare_api_token.is_empty() {
            self.ui.warning(
                "No Cloudflare API token was supplied; using Let's Encrypt with ZeroSSL fallback.",
            );
            config.tls_mode = TlsMode::PublicAcme;
        } else {
            config.tls_mode = TlsMode::CloudflareOrigin;
        }
        Ok(())
    }

    pub(crate) fn provision_tls(&self, config: &ControlConfig) -> Result<()> {
        if !matches!(config.tls_mode, TlsMode::CloudflareOrigin) {
            return Ok(());
        }
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .context("build Cloudflare API client")?;
        let response = runtime()?.block_on(async {
            client
                .post(ORIGIN_CA_ENDPOINT)
                .bearer_auth(&config.cloudflare_api_token)
                .json(&json!({
                    "hostnames": [&config.domain, &config.content_domain],
                    "request_type": "origin-ecc",
                    "requested_validity": 5475
                }))
                .send()
                .await
        })?;
        let status = response.status();
        let body: Value = runtime()?
            .block_on(response.json())
            .context("parse Cloudflare response")?;
        if status != StatusCode::OK || body.get("success").and_then(Value::as_bool) != Some(true) {
            let detail = body.get("errors").cloned().unwrap_or(Value::Null);
            bail!("Cloudflare Origin CA issuance failed ({status}): {detail}");
        }
        let result = body
            .get("result")
            .context("Cloudflare response omitted result")?;
        let certificate = result
            .get("certificate")
            .and_then(Value::as_str)
            .context("Cloudflare response omitted certificate")?;
        let private_key = result
            .get("private_key")
            .and_then(Value::as_str)
            .context("Cloudflare response omitted private key")?;
        let tls_dir = self.config_dir.join("tls");
        fs::create_dir_all(&tls_dir)?;
        fs::write(tls_dir.join("origin.crt"), certificate)?;
        fs::write(tls_dir.join("origin.key"), private_key)?;
        fs::set_permissions(
            tls_dir.join("origin.crt"),
            fs::Permissions::from_mode(0o644),
        )?;
        fs::set_permissions(
            tls_dir.join("origin.key"),
            fs::Permissions::from_mode(0o600),
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn origin_endpoint_is_https() {
        assert!(ORIGIN_CA_ENDPOINT.starts_with("https://"));
    }
}
