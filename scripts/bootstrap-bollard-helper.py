from pathlib import Path


def replace(path: str, old: str, new: str) -> None:
    p = Path(path)
    text = p.read_text()
    if old not in text:
        raise SystemExit(f"expected block not found in {path}: {old[:80]!r}")
    p.write_text(text.replace(old, new, 1))


# Helper library: expose the Bollard adapter and route Docker operations through it.
replace(
    "crates/helper/src/lib.rs",
    "use protocol::{BackupArtifact, BackupState, SecurityFinding, SecurityState};\n",
    "pub mod docker;\n\nuse protocol::{BackupArtifact, BackupState, SecurityFinding, SecurityState};\n",
)
replace(
    "crates/helper/src/lib.rs",
    "const MAX_DOCKER_OUTPUT_BYTES: usize = 256 * 1024;\n",
    "",
)
old_docker = '''    pub async fn docker_list(&self) -> Result<String, HelperError> {
        Ok(truncate_utf8(
            run_capture(
                "docker",
                &["ps", "-a", "--no-trunc", "--format", "{{json .}}"],
            )
            .await?,
            MAX_DOCKER_OUTPUT_BYTES,
        ))
    }
    pub async fn docker_start(&self, container: &str) -> Result<(), HelperError> {
        self.docker_action("start", container).await
    }
    pub async fn docker_stop(&self, container: &str) -> Result<(), HelperError> {
        self.docker_action("stop", container).await
    }
    pub async fn docker_restart(&self, container: &str) -> Result<(), HelperError> {
        self.docker_action("restart", container).await
    }
    async fn docker_action(&self, action: &str, container: &str) -> Result<(), HelperError> {
        Self::validate_container_reference(container)?;
        run("docker", &[action, container]).await
    }
'''
new_docker = '''    pub async fn docker_list(&self) -> Result<String, HelperError> {
        docker::list().await
    }
    pub async fn docker_inspect(&self, container: &str) -> Result<String, HelperError> {
        Self::validate_container_reference(container)?;
        docker::inspect(container).await
    }
    pub async fn docker_stats(&self, container: &str) -> Result<String, HelperError> {
        Self::validate_container_reference(container)?;
        docker::stats(container).await
    }
    pub async fn docker_logs(&self, container: &str, lines: u32) -> Result<String, HelperError> {
        Self::validate_container_reference(container)?;
        if lines == 0 || lines > MAX_JOURNAL_LINES {
            return Err(HelperError::InvalidRequest);
        }
        docker::logs(container, lines).await
    }
    pub async fn docker_start(&self, container: &str) -> Result<(), HelperError> {
        Self::validate_container_reference(container)?;
        docker::start(container).await
    }
    pub async fn docker_stop(&self, container: &str) -> Result<(), HelperError> {
        Self::validate_container_reference(container)?;
        docker::stop(container).await
    }
    pub async fn docker_restart(&self, container: &str) -> Result<(), HelperError> {
        Self::validate_container_reference(container)?;
        docker::restart(container).await
    }
'''
replace("crates/helper/src/lib.rs", old_docker, new_docker)

# Protocol: expose read-only Docker operations through the existing command pipeline.
replace("crates/protocol/src/lib.rs", 'pub const PROTOCOL_VERSION: &str = "1.10";', 'pub const PROTOCOL_VERSION: &str = "1.11";')
replace(
    "crates/protocol/src/lib.rs",
    '''    #[serde(rename = "docker.start")]
    DockerStart { container: String },
''',
    '''    #[serde(rename = "docker.inspect")]
    DockerInspect { container: String },
    #[serde(rename = "docker.stats")]
    DockerStats { container: String },
    #[serde(rename = "docker.logs")]
    DockerLogs { container: String, lines: u32 },
    #[serde(rename = "docker.start")]
    DockerStart { container: String },
''',
)
replace(
    "crates/protocol/src/lib.rs",
    '''            CommandType::LogsJournal { .. } => "logs.read",
            CommandType::DockerStart { .. }
''',
    '''            CommandType::LogsJournal { .. } => "logs.read",
            CommandType::DockerInspect { .. }
            | CommandType::DockerStats { .. }
            | CommandType::DockerLogs { .. } => "docker.read",
            CommandType::DockerStart { .. }
''',
)
replace(
    "crates/protocol/src/lib.rs",
    '''            CommandType::DockerStart { .. }
            | CommandType::DockerStop { .. }
            | CommandType::DockerRestart { .. } => Capability {
''',
    '''            CommandType::DockerInspect { .. }
            | CommandType::DockerStats { .. }
            | CommandType::DockerLogs { .. }
            | CommandType::DockerStart { .. }
            | CommandType::DockerStop { .. }
            | CommandType::DockerRestart { .. } => Capability {
''',
)
replace(
    "crates/protocol/src/lib.rs",
    '''    #[serde(rename = "docker.list")]
    DockerList,
    #[serde(rename = "docker.start")]
''',
    '''    #[serde(rename = "docker.list")]
    DockerList,
    #[serde(rename = "docker.inspect")]
    DockerInspect { container: String },
    #[serde(rename = "docker.stats")]
    DockerStats { container: String },
    #[serde(rename = "docker.logs")]
    DockerLogs { container: String, lines: u32 },
    #[serde(rename = "docker.start")]
''',
)

# Helper binary dispatch.
replace(
    "crates/helper/src/main.rs",
    '''        HelperRequest::DockerList => api.docker_list().await.map(Some),
        HelperRequest::DockerStart { container } => {
''',
    '''        HelperRequest::DockerList => api.docker_list().await.map(Some),
        HelperRequest::DockerInspect { container } => api.docker_inspect(&container).await.map(Some),
        HelperRequest::DockerStats { container } => api.docker_stats(&container).await.map(Some),
        HelperRequest::DockerLogs { container, lines } => {
            api.docker_logs(&container, lines).await.map(Some)
        }
        HelperRequest::DockerStart { container } => {
''',
)

# Agent client + command execution.
replace(
    "crates/agent/src/lib.rs",
    '''    pub async fn docker_list(&self) -> Result<String, OperationError> {
        Ok(self
            .request(HelperRequest::DockerList)
            .await?
            .unwrap_or_default())
    }
    pub async fn docker_start(&self, container: &str) -> Result<(), OperationError> {
''',
    '''    pub async fn docker_list(&self) -> Result<String, OperationError> {
        Ok(self
            .request(HelperRequest::DockerList)
            .await?
            .unwrap_or_default())
    }
    pub async fn docker_inspect(&self, container: &str) -> Result<String, OperationError> {
        Ok(self
            .request(HelperRequest::DockerInspect {
                container: container.into(),
            })
            .await?
            .unwrap_or_default())
    }
    pub async fn docker_stats(&self, container: &str) -> Result<String, OperationError> {
        Ok(self
            .request(HelperRequest::DockerStats {
                container: container.into(),
            })
            .await?
            .unwrap_or_default())
    }
    pub async fn docker_logs(&self, container: &str, lines: u32) -> Result<String, OperationError> {
        Ok(self
            .request(HelperRequest::DockerLogs {
                container: container.into(),
                lines,
            })
            .await?
            .unwrap_or_default())
    }
    pub async fn docker_start(&self, container: &str) -> Result<(), OperationError> {
''',
)
replace(
    "crates/agent/src/lib.rs",
    '''            protocol::CommandType::DockerStart { container } => {
                self.helper.docker_start(container).await.map(|_| None)
            }
''',
    '''            protocol::CommandType::DockerInspect { container } => {
                self.helper.docker_inspect(container).await.map(Some)
            }
            protocol::CommandType::DockerStats { container } => {
                self.helper.docker_stats(container).await.map(Some)
            }
            protocol::CommandType::DockerLogs { container, lines } => {
                self.helper.docker_logs(container, *lines).await.map(Some)
            }
            protocol::CommandType::DockerStart { container } => {
                self.helper.docker_start(container).await.map(|_| None)
            }
''',
)

# Documentation: make the runtime boundary explicit.
path = Path("docs/development.md")
text = path.read_text()
needle = "Do not run the Helper as a network service or add generic shell execution to simplify development.\n"
addition = """Do not run the Helper as a network service or add generic shell execution to simplify development.\n\nDocker resource operations in the Helper use Bollard and the local Docker Engine API for typed container listing, inspection, stats, logs, protection-label checks, and start/stop/restart. Docker Compose remains the orchestration boundary for stack-level operations; do not reimplement Compose semantics in Bollard.\n"""
if needle not in text:
    raise SystemExit("development Helper boundary marker missing")
path.write_text(text.replace(needle, addition, 1))
