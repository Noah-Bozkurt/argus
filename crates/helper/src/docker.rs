use bollard::{
    Docker,
    container::LogOutput,
    errors::Error as BollardError,
    query_parameters::{ListContainersOptionsBuilder, LogsOptionsBuilder, StatsOptionsBuilder},
};
use futures_util::StreamExt;
use serde_json::json;
use std::collections::HashMap;

use crate::HelperError;

const PROTECTED_LABEL: &str = "com.argus.protected";
const COMPOSE_PROJECT_LABEL: &str = "com.docker.compose.project";
const MAX_DOCKER_OUTPUT_BYTES: usize = 256 * 1024;

fn client() -> Result<Docker, HelperError> {
    Docker::connect_with_local_defaults().map_err(map_error)
}

fn map_error(error: BollardError) -> HelperError {
    match error {
        BollardError::DockerResponseServerError {
            status_code: 404, ..
        } => HelperError::InvalidContainerReference,
        other => HelperError::SystemCommandFailed(format!("Docker Engine API request failed: {other}")),
    }
}

fn format_ports(ports: Option<Vec<bollard::models::PortSummary>>) -> String {
    ports
        .unwrap_or_default()
        .into_iter()
        .map(|port| match (port.ip.as_deref(), port.public_port) {
            (Some(ip), Some(public)) if !ip.is_empty() => {
                format!("{ip}:{public}->{}", port.private_port)
            }
            (_, Some(public)) => format!("{public}->{}", port.private_port),
            _ => port.private_port.to_string(),
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn container_name(names: Option<Vec<String>>) -> String {
    names
        .unwrap_or_default()
        .into_iter()
        .next()
        .unwrap_or_default()
        .trim_start_matches('/')
        .to_string()
}

fn state_name(state: Option<bollard::models::ContainerSummaryStateEnum>) -> String {
    state
        .map(|state| format!("{state:?}").to_ascii_lowercase())
        .unwrap_or_else(|| "unknown".to_string())
}

fn log_bytes(output: LogOutput) -> Vec<u8> {
    match output {
        LogOutput::StdErr { message }
        | LogOutput::StdOut { message }
        | LogOutput::StdIn { message }
        | LogOutput::Console { message } => message.to_vec(),
    }
}

fn truncate_bytes(buffer: &mut Vec<u8>) {
    if buffer.len() > MAX_DOCKER_OUTPUT_BYTES {
        let excess = buffer.len() - MAX_DOCKER_OUTPUT_BYTES;
        buffer.drain(..excess);
    }
}

pub async fn list() -> Result<String, HelperError> {
    let docker = client()?;
    let options = ListContainersOptionsBuilder::default().all(true).build();
    let containers = docker
        .list_containers(Some(options))
        .await
        .map_err(map_error)?;

    let mut lines = Vec::new();
    for container in containers.into_iter().take(500) {
        lines.push(
            serde_json::to_string(&json!({
                "ID": container.id.unwrap_or_default(),
                "Names": container_name(container.names),
                "Image": container.image.unwrap_or_default(),
                "State": state_name(container.state),
                "Status": container.status.unwrap_or_else(|| "unknown".to_string()),
                "Ports": format_ports(container.ports),
            }))
            .map_err(|error| HelperError::SystemCommandFailed(error.to_string()))?,
        );
    }
    Ok(lines.join("\n"))
}

pub async fn start(container: &str) -> Result<(), HelperError> {
    client()?
        .start_container(container, None)
        .await
        .map_err(map_error)
}

pub async fn stop(container: &str) -> Result<(), HelperError> {
    client()?
        .stop_container(container, None)
        .await
        .map_err(map_error)
}

pub async fn restart(container: &str) -> Result<(), HelperError> {
    client()?
        .restart_container(container, None)
        .await
        .map_err(map_error)
}

pub async fn inspect(container: &str) -> Result<String, HelperError> {
    let inspected = client()?
        .inspect_container(container, None)
        .await
        .map_err(map_error)?;
    serde_json::to_string(&inspected)
        .map_err(|error| HelperError::SystemCommandFailed(error.to_string()))
}

pub async fn stats(container: &str) -> Result<String, HelperError> {
    let docker = client()?;
    let options = StatsOptionsBuilder::default()
        .stream(false)
        .one_shot(true)
        .build();
    let mut stream = docker.stats(container, Some(options));
    let stats = stream
        .next()
        .await
        .ok_or_else(|| {
            HelperError::SystemCommandFailed("Docker stats returned no sample".to_string())
        })?
        .map_err(map_error)?;
    serde_json::to_string(&stats)
        .map_err(|error| HelperError::SystemCommandFailed(error.to_string()))
}

pub async fn logs(container: &str, lines: u32) -> Result<String, HelperError> {
    let docker = client()?;
    let tail = lines.to_string();
    let options = LogsOptionsBuilder::default()
        .follow(false)
        .stdout(true)
        .stderr(true)
        .timestamps(true)
        .tail(&tail)
        .build();
    let mut stream = docker.logs(container, Some(options));
    let mut buffer = Vec::new();
    while let Some(output) = stream.next().await {
        buffer.extend_from_slice(&log_bytes(output.map_err(map_error)?));
        truncate_bytes(&mut buffer);
    }
    Ok(String::from_utf8_lossy(&buffer).into_owned())
}

pub async fn container_is_protected(container: &str) -> Result<bool, HelperError> {
    let inspected = client()?
        .inspect_container(container, None)
        .await
        .map_err(map_error)?;
    Ok(inspected
        .config
        .and_then(|config| config.labels)
        .and_then(|labels| labels.get(PROTECTED_LABEL).cloned())
        .is_some_and(|value| value.eq_ignore_ascii_case("true")))
}

pub async fn compose_project_is_protected(project: &str) -> Result<bool, HelperError> {
    let docker = client()?;
    let mut filters = HashMap::new();
    filters.insert(
        "label".to_string(),
        vec![format!("{COMPOSE_PROJECT_LABEL}={project}")],
    );
    let options = ListContainersOptionsBuilder::default()
        .all(true)
        .filters(&filters)
        .build();
    let containers = docker
        .list_containers(Some(options))
        .await
        .map_err(map_error)?;
    Ok(containers.into_iter().any(|container| {
        container
            .labels
            .and_then(|labels| labels.get(PROTECTED_LABEL).cloned())
            .is_some_and(|value| value.eq_ignore_ascii_case("true"))
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn container_names_drop_docker_slash_prefix() {
        assert_eq!(container_name(Some(vec!["/postgres".into()])), "postgres");
    }

    #[test]
    fn port_summary_is_human_readable() {
        let ports = vec![bollard::models::PortSummary {
            ip: Some("0.0.0.0".into()),
            private_port: 80,
            public_port: Some(8080),
            typ: None,
        }];
        assert_eq!(format_ports(Some(ports)), "0.0.0.0:8080->80");
    }
}
