use super::installer_shared::{ControlConfig, Installer, TlsMode};
use anyhow::{Context, Result, bail};
use cli::lifecycle::{self, prompt_line, prompt_secret, temp_dir, write_env_file};
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

fn origin_ca_request(domain: &str, content_domain: &str, csr: &str) -> Value {
    json!({
        "csr": csr,
        "hostnames": [domain, content_domain],
        "request_type": "origin-ecc",
        "requested_validity": 5475
    })
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
        let use_cloudflare = if lifecycle::interactive_available() {
            println!("\nCloudflare proxy detected for both domains.");
            println!("  1. Cloudflare Origin CA (requires a Cloudflare API token)");
            println!("  2. Let's Encrypt with ZeroSSL fallback\n");
            loop {
                match prompt_line("Choose certificate provider [1-2]: ")?.trim() {
                    "1" => break true,
                    "2" => break false,
                    _ => self.ui.warning("Choose 1 or 2."),
                }
            }
        } else {
            std::env::var("ARGUS_TLS_MODE").as_deref() == Ok("cloudflare-origin")
                || !config.cloudflare_api_token.is_empty()
        };
        if !use_cloudflare {
            config.tls_mode = TlsMode::PublicAcme;
            return Ok(());
        }

        if config.cloudflare_api_token.is_empty() && lifecycle::interactive_available() {
            println!("The token needs Zone / SSL and Certificates / Edit permission.");
            println!(
                "It will be saved root-only for certificate repair and domain changes. Keep Cloudflare SSL/TLS mode set to Full (strict).\n"
            );
            config.cloudflare_api_token = prompt_secret("Cloudflare API token: ")?;
        }
        if config.cloudflare_api_token.is_empty() {
            bail!("Cloudflare Origin CA was selected but no API token was supplied");
        } else {
            config.tls_mode = TlsMode::CloudflareOrigin;
        }
        Ok(())
    }

    pub(crate) fn provision_tls(&self, config: &ControlConfig) -> Result<()> {
        if !matches!(config.tls_mode, TlsMode::CloudflareOrigin) {
            return Ok(());
        }
        let tls_dir = self.config_dir.join("tls");
        if config.existing_install
            && tls_dir.join("origin.crt").is_file()
            && tls_dir.join("origin.key").is_file()
        {
            return Ok(());
        }
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .context("build Cloudflare API client")?;
        let work_dir = temp_dir("argus-origin-ca")?;
        let private_key_path = work_dir.join("origin.key");
        let csr_path = work_dir.join("origin.csr");
        let private_key_arg = private_key_path.display().to_string();
        let csr_arg = csr_path.display().to_string();
        let subject = format!("/CN={}", config.domain);
        let subject_alt_names = format!(
            "subjectAltName=DNS:{},DNS:{}",
            config.domain, config.content_domain
        );
        let issuance = (|| -> Result<(StatusCode, Value, Vec<u8>)> {
            lifecycle::run_quiet(
                "openssl",
                &[
                    "genpkey",
                    "-algorithm",
                    "EC",
                    "-pkeyopt",
                    "ec_paramgen_curve:P-256",
                    "-out",
                    &private_key_arg,
                ],
            )
            .context("generate Cloudflare Origin CA private key")?;
            lifecycle::run_quiet(
                "openssl",
                &[
                    "req",
                    "-new",
                    "-sha256",
                    "-key",
                    &private_key_arg,
                    "-out",
                    &csr_arg,
                    "-subj",
                    &subject,
                    "-addext",
                    &subject_alt_names,
                ],
            )
            .context("generate Cloudflare Origin CA certificate request")?;
            let csr = fs::read_to_string(&csr_path)
                .context("read Cloudflare Origin CA certificate request")?;
            let private_key =
                fs::read(&private_key_path).context("read Cloudflare Origin CA private key")?;
            let (status, body) = runtime()?.block_on(async {
                let response = client
                    .post(ORIGIN_CA_ENDPOINT)
                    .bearer_auth(&config.cloudflare_api_token)
                    .json(&origin_ca_request(
                        &config.domain,
                        &config.content_domain,
                        &csr,
                    ))
                    .send()
                    .await?;
                let status = response.status();
                let body = response
                    .json::<Value>()
                    .await
                    .context("parse Cloudflare response")?;
                Ok::<_, anyhow::Error>((status, body))
            })?;
            Ok((status, body, private_key))
        })();
        let cleanup = fs::remove_dir_all(&work_dir).context("remove temporary TLS files");
        let (status, body, private_key) = issuance?;
        cleanup?;
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
        write_env_file(
            &self.config_dir.join("cloudflare.env"),
            &[(
                "ARGUS_CLOUDFLARE_API_TOKEN",
                config.cloudflare_api_token.as_str(),
            )],
            0o600,
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

    #[test]
    fn origin_request_includes_csr_and_both_hostnames() {
        let request = origin_ca_request(
            "app.example.com",
            "content.example.com",
            "-----BEGIN CERTIFICATE REQUEST-----",
        );
        assert_eq!(
            request.get("csr").and_then(Value::as_str),
            Some("-----BEGIN CERTIFICATE REQUEST-----")
        );
        assert_eq!(
            request.get("hostnames"),
            Some(&json!(["app.example.com", "content.example.com"]))
        );
    }
}
