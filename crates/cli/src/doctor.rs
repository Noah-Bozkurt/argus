use cli::{domain, lifecycle};
use serde::Serialize;
use std::{
    fs,
    os::{unix::fs::PermissionsExt, unix::net::UnixStream},
    path::Path,
    process::Command,
    time::Duration,
};

#[derive(Debug, Serialize)]
pub(crate) struct DoctorCheck {
    name: String,
    status: &'static str,
    detail: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    remedy: Option<String>,
}

#[derive(Debug, Serialize)]
pub(crate) struct DoctorReport {
    pub(crate) healthy: bool,
    checks: Vec<DoctorCheck>,
}

impl DoctorReport {
    fn push_ok(&mut self, name: &str, detail: impl Into<String>) {
        self.checks.push(DoctorCheck {
            name: name.to_string(),
            status: "ok",
            detail: detail.into(),
            remedy: None,
        });
    }

    fn push_failed(&mut self, name: &str, detail: impl Into<String>, remedy: impl Into<String>) {
        self.healthy = false;
        self.checks.push(DoctorCheck {
            name: name.to_string(),
            status: "failed",
            detail: detail.into(),
            remedy: Some(remedy.into()),
        });
    }

    fn push_skipped(&mut self, name: &str, detail: impl Into<String>) {
        self.checks.push(DoctorCheck {
            name: name.to_string(),
            status: "skipped",
            detail: detail.into(),
            remedy: None,
        });
    }

    pub(crate) fn print_human(&self, verbose: bool) {
        println!("Argus doctor\n");
        for check in &self.checks {
            let marker = match check.status {
                "ok" => "✓",
                "failed" => "✗",
                _ => "–",
            };
            println!("  {marker} {}: {}", check.name, check.detail);
            if let Some(remedy) = &check.remedy {
                println!("    Next: {remedy}");
            } else if verbose && check.status == "skipped" {
                println!("    This check was intentionally skipped.");
            }
        }
        let failures = self
            .checks
            .iter()
            .filter(|check| check.status == "failed")
            .count();
        println!();
        if self.healthy {
            println!("Result: healthy");
        } else {
            println!("Result: unhealthy — {failures} problem(s) found");
        }
    }
}

fn command_output(program: &str, args: &[&str]) -> Option<String> {
    Command::new(program)
        .args(args)
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn compose_output(install_dir: &Path, args: &[&str]) -> Option<String> {
    let mut values = vec![
        "compose".to_string(),
        "--project-directory".to_string(),
        install_dir.display().to_string(),
        "--env-file".to_string(),
        install_dir.join(".env").display().to_string(),
        "-f".to_string(),
        install_dir.join("compose.yaml").display().to_string(),
    ];
    values.extend(args.iter().map(|value| (*value).to_string()));
    let refs = values.iter().map(String::as_str).collect::<Vec<_>>();
    command_output("docker", &refs)
}

fn service_active(name: &str) -> bool {
    Command::new("systemctl")
        .args(["is-active", "--quiet", name])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

fn https_status(domain: &str) -> Option<String> {
    command_output(
        "curl",
        &[
            "-sS",
            "--connect-timeout",
            "5",
            "--max-time",
            "10",
            "-o",
            "/dev/null",
            "-w",
            "%{http_code}",
            &format!("https://{domain}/healthz"),
        ],
    )
}

pub(crate) async fn run(offline: bool) -> DoctorReport {
    let mut report = DoctorReport {
        healthy: true,
        checks: Vec::new(),
    };
    let install_dir = lifecycle::env_path("ARGUS_INSTALL_DIR", lifecycle::DEFAULT_INSTALL_DIR);
    let config_dir = lifecycle::env_path("ARGUS_CONFIG_DIR", lifecycle::DEFAULT_CONFIG_DIR);
    let state_dir = lifecycle::env_path("ARGUS_STATE_DIR", lifecycle::DEFAULT_STATE_DIR);

    let agent_config = state_dir.join("agent.json");
    let control_plane = install_dir.join(".env").is_file();
    let required = if control_plane {
        vec![install_dir.join(".env"), install_dir.join("compose.yaml")]
    } else {
        vec![
            Path::new("/usr/local/bin/argus-agent").to_path_buf(),
            Path::new("/usr/local/bin/argus-helper").to_path_buf(),
            config_dir.join("agent.env"),
            config_dir.join("helper.env"),
        ]
    };
    let missing = required
        .iter()
        .filter(|path| !path.is_file())
        .map(|path| path.display().to_string())
        .collect::<Vec<_>>();
    if missing.is_empty() {
        report.push_ok(
            "Installation files",
            if control_plane {
                format!("control plane at {}", install_dir.display())
            } else {
                "managed-node files are present".to_string()
            },
        );
    } else {
        report.push_failed(
            "Installation files",
            format!("missing {}", missing.join(", ")),
            "run sudo argusctl repair, or use the public installer if argusctl is damaged",
        );
    }

    for (label, path) in [("Runtime configuration", install_dir.join(".env"))] {
        if let Ok(metadata) = fs::metadata(&path) {
            let mode = metadata.permissions().mode() & 0o777;
            if mode == 0o600 {
                report.push_ok(label, format!("{} has mode 0600", path.display()));
            } else {
                report.push_failed(
                    label,
                    format!("{} has insecure mode {mode:04o}", path.display()),
                    "run sudo argusctl repair to restore secure permissions",
                );
            }
        }
    }

    for (label, service, logs) in [
        (
            "Agent service",
            "argus-agent.service",
            "argusctl logs agent",
        ),
        (
            "Helper service",
            "argus-helper.service",
            "argusctl logs helper",
        ),
    ] {
        if service_active(service) {
            report.push_ok(label, "active");
        } else {
            report.push_failed(
                label,
                "not active",
                format!("inspect {logs}; then run sudo argusctl repair"),
            );
        }
    }

    if agent_config.is_file() {
        report.push_ok("Agent identity", agent_config.display().to_string());
        match agent::AgentConfig::load(&agent_config).await {
            Ok(config) => {
                if UnixStream::connect(&config.helper_socket).is_ok() {
                    report.push_ok("Helper socket", config.helper_socket.display().to_string());
                } else {
                    report.push_failed(
                        "Helper socket",
                        format!("{} is not reachable", config.helper_socket.display()),
                        "inspect argusctl logs helper and run sudo argusctl repair",
                    );
                }
                if offline {
                    report.push_skipped("Agent connection", "offline mode");
                } else if let Some(credential) = config.credential.as_deref() {
                    let result = match reqwest::Client::builder()
                        .timeout(Duration::from_secs(10))
                        .build()
                    {
                        Ok(client) => client
                            .get(format!("{}/agent/identity", config.control_plane_url))
                            .bearer_auth(credential)
                            .send()
                            .await
                            .ok()
                            .map(|response| response.status()),
                        Err(_) => None,
                    };
                    match result {
                        Some(status) if status.is_success() => {
                            report.push_ok("Agent connection", "authenticated");
                        }
                        Some(status) => report.push_failed(
                            "Agent connection",
                            format!("control plane returned {status}"),
                            "check the configured URL and re-enroll the Agent if its credential was revoked",
                        ),
                        None => report.push_failed(
                            "Agent connection",
                            "control plane is unreachable",
                            "check DNS, HTTPS, and the configured control-plane URL",
                        ),
                    }
                } else {
                    report.push_failed(
                        "Agent connection",
                        "Agent credential is missing",
                        "repair or re-enroll this host",
                    );
                }
            }
            Err(error) => report.push_failed(
                "Agent configuration",
                error.to_string(),
                "run sudo argusctl repair or re-enroll this host",
            ),
        }
    } else {
        report.push_failed(
            "Agent identity",
            format!("{} is missing", agent_config.display()),
            "repair or re-enroll this host",
        );
    }

    if lifecycle::command_exists("docker") {
        let compose = install_dir.join("compose.yaml");
        let env = install_dir.join(".env");
        if compose.is_file() && env.is_file() {
            let output = compose_output(&install_dir, &["ps", "--format", "json"]);
            match output {
                Some(value) if !value.is_empty() => {
                    let unhealthy = value.lines().any(|line| {
                        line.contains("unhealthy")
                            || line.contains("exited")
                            || line.contains("dead")
                    });
                    if unhealthy {
                        report.push_failed(
                            "Control-plane containers",
                            "one or more containers are unhealthy or stopped",
                            "inspect argusctl logs control-plane; then run sudo argusctl repair",
                        );
                    } else {
                        report.push_ok("Control-plane containers", "running");
                    }
                }
                _ => report.push_failed(
                    "Control-plane containers",
                    "Compose status is unavailable",
                    "check Docker, inspect argusctl logs control-plane, then run sudo argusctl repair",
                ),
            }
        } else {
            report.push_skipped(
                "Control-plane containers",
                "no control-plane deployment found",
            );
        }
    } else if control_plane {
        report.push_failed(
            "Docker",
            "docker is unavailable",
            "install or start Docker, then run sudo argusctl repair",
        );
    } else {
        report.push_skipped("Docker", "not required for local Agent diagnostics");
    }

    let disk_path = if install_dir.exists() {
        install_dir.as_path()
    } else {
        Path::new("/")
    };
    match command_output("df", &["-h", disk_path.to_str().unwrap_or("/")]) {
        Some(output) => report.push_ok(
            "Disk",
            output.lines().last().unwrap_or("available").to_string(),
        ),
        None => report.push_skipped("Disk", "capacity could not be read"),
    }

    if offline {
        report.push_skipped("Public endpoints", "offline mode");
    } else if !control_plane {
        report.push_skipped(
            "Public endpoints",
            "managed node connectivity is covered by the Agent connection check",
        );
    } else {
        match domain::installed_domains() {
            Ok(domains) => {
                let caddy_logs = if control_plane {
                    compose_output(&install_dir, &["logs", "--tail=200", "caddy"])
                        .unwrap_or_default()
                } else {
                    String::new()
                };
                let tls_error = domain::caddy_tls_error(
                    &caddy_logs,
                    &[domains.web.as_str(), domains.content.as_str()],
                );
                for (label, value) in [
                    ("Web HTTPS", domains.web),
                    ("Content HTTPS", domains.content),
                ] {
                    match domain::resolve_domain(&value) {
                        Ok(addresses) => match https_status(&value) {
                            Some(code)
                                if matches!(code.as_str(), "200" | "302" | "307" | "308") =>
                            {
                                report.push_ok(
                                    label,
                                    format!(
                                        "{value} returned HTTP {code} via {} DNS address(es)",
                                        addresses.len()
                                    ),
                                );
                            }
                            Some(code) => report.push_failed(
                                label,
                                format!("{value} returned HTTP {code}"),
                                "inspect argusctl logs caddy and DNS configuration",
                            ),
                            None => report.push_failed(
                                label,
                                tls_error.clone().unwrap_or_else(|| {
                                    format!("{value} is not reachable with trusted HTTPS")
                                }),
                                if tls_error.is_some() {
                                    "wait until the listed retry time; reinstalling cannot bypass the CA limit"
                                } else {
                                    "inspect DNS, firewall access, and argusctl logs caddy"
                                },
                            ),
                        },
                        Err(error) => report.push_failed(
                            label,
                            error.to_string(),
                            "configure DNS before retrying",
                        ),
                    }
                }
            }
            Err(error) => report.push_failed(
                "Public endpoints",
                error.to_string(),
                "restore the installed environment with sudo argusctl repair",
            ),
        }
    }

    let recovery_root = state_dir.join("update-backups");
    if Path::new(&recovery_root).is_dir() {
        report.push_ok("Update recovery", recovery_root.display().to_string());
    } else {
        report.push_skipped("Update recovery", "no update snapshots recorded");
    }

    report
}
