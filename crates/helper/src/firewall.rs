use helper::HelperError;
use std::collections::BTreeSet;
use tokio::process::Command;

const ROLLBACK_SECONDS: &str = "120s";
const UFW_PATH: &str = "/usr/sbin/ufw";

pub async fn enable(rollback_id: &str) -> Result<(), HelperError> {
    validate_rollback_id(rollback_id)?;
    ensure_ufw_available().await?;

    if firewall_is_active().await? {
        return Ok(());
    }

    let sshd = run_capture("sshd", &["-T"]).await?;
    let ports = parse_sshd_ports(&sshd)?;
    for port in ports {
        let rule = format!("{port}/tcp");
        run(
            "ufw",
            &["allow", &rule, "comment", "Argus SSH safety rule"],
        )
        .await?;
    }

    schedule_rollback(rollback_id).await?;
    if let Err(error) = run("ufw", &["--force", "enable"]).await {
        let _ = commit(rollback_id).await;
        return Err(error);
    }

    if !firewall_is_active().await? {
        return Err(HelperError::SystemCommandFailed(
            "UFW did not report active after enable; rollback remains armed".into(),
        ));
    }
    Ok(())
}

pub async fn commit(rollback_id: &str) -> Result<(), HelperError> {
    validate_rollback_id(rollback_id)?;
    let timer = format!("{}.timer", rollback_unit(rollback_id));
    let status = Command::new("systemctl")
        .args(["is-active", "--quiet", &timer])
        .status()
        .await
        .map_err(map_spawn("systemctl"))?;
    if status.success() {
        run("systemctl", &["stop", &timer]).await?;
    }
    Ok(())
}

async fn ensure_ufw_available() -> Result<(), HelperError> {
    if tokio::fs::metadata(UFW_PATH).await.is_err() {
        return Err(HelperError::UtilityUnavailable("ufw".into()));
    }
    run_capture("ufw", &["version"]).await.map(|_| ())
}

async fn firewall_is_active() -> Result<bool, HelperError> {
    let output = run_capture("ufw", &["status"]).await?;
    Ok(output
        .lines()
        .next()
        .and_then(|line| line.strip_prefix("Status: "))
        .is_some_and(|value| value.trim().eq_ignore_ascii_case("active")))
}

async fn schedule_rollback(rollback_id: &str) -> Result<(), HelperError> {
    let unit = rollback_unit(rollback_id);
    run(
        "systemd-run",
        &[
            "--unit",
            &unit,
            "--on-active",
            ROLLBACK_SECONDS,
            UFW_PATH,
            "--force",
            "disable",
        ],
    )
    .await
}

fn rollback_unit(rollback_id: &str) -> String {
    format!("argus-firewall-rollback-{rollback_id}")
}

fn validate_rollback_id(rollback_id: &str) -> Result<(), HelperError> {
    let valid = !rollback_id.is_empty()
        && rollback_id.len() <= 64
        && rollback_id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-');
    if valid {
        Ok(())
    } else {
        Err(HelperError::InvalidRequest)
    }
}

fn parse_sshd_ports(output: &str) -> Result<Vec<u16>, HelperError> {
    let ports = output
        .lines()
        .filter_map(|line| line.strip_prefix("port "))
        .filter_map(|value| value.trim().parse::<u16>().ok())
        .filter(|port| *port != 0)
        .collect::<BTreeSet<_>>();
    if ports.is_empty() {
        Err(HelperError::SystemCommandFailed(
            "could not determine effective SSH port; refusing to enable firewall".into(),
        ))
    } else {
        Ok(ports.into_iter().collect())
    }
}

async fn run(program: &str, args: &[&str]) -> Result<(), HelperError> {
    run_capture(program, args).await.map(|_| ())
}

async fn run_capture(program: &str, args: &[&str]) -> Result<String, HelperError> {
    let output = Command::new(program)
        .args(args)
        .output()
        .await
        .map_err(map_spawn(program))?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    } else {
        Err(HelperError::SystemCommandFailed(
            String::from_utf8_lossy(&output.stderr).trim().to_string(),
        ))
    }
}

fn map_spawn(program: &str) -> impl FnOnce(std::io::Error) -> HelperError + '_ {
    move |error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            HelperError::UtilityUnavailable(program.into())
        } else {
            HelperError::SystemCommandFailed(error.to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_effective_ssh_ports_without_duplicates() {
        let ports = parse_sshd_ports("passwordauthentication no\nport 2222\nport 22\nport 2222\n")
            .unwrap();
        assert_eq!(ports, vec![22, 2222]);
    }

    #[test]
    fn refuses_firewall_enable_without_detectable_ssh_port() {
        assert!(parse_sshd_ports("passwordauthentication no\n").is_err());
    }

    #[test]
    fn rollback_ids_cannot_escape_systemd_unit_name() {
        assert!(validate_rollback_id("550e8400-e29b-41d4-a716-446655440000").is_ok());
        for invalid in ["", "../unit", "x;reboot", "unit name"] {
            assert!(validate_rollback_id(invalid).is_err());
        }
    }
}
