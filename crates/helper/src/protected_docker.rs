use protocol::{HelperRequest, OperationError};
use tokio::process::Command;

const PROTECTED_LABEL: &str = "com.argus.protected";

fn denied() -> OperationError {
    OperationError {
        code: "PERMISSION_DENIED".into(),
        message: "Argus control-plane containers are protected from managed Docker actions".into(),
    }
}

async fn container_is_protected(container: &str) -> bool {
    let format = format!("{{{{ index .Config.Labels \"{PROTECTED_LABEL}\" }}}}");
    let output = Command::new("docker")
        .args(["inspect", "--format", &format, container])
        .output()
        .await;
    let Ok(output) = output else {
        return false;
    };
    output.status.success() && String::from_utf8_lossy(&output.stdout).trim() == "true"
}

async fn compose_project_is_protected(project: &str) -> bool {
    if project.is_empty() || project.len() > 128 {
        return false;
    }
    let filter = format!("label=com.docker.compose.project={project}");
    let output = Command::new("docker")
        .args(["ps", "-a", "--filter", &filter, "--format", "{{.ID}}"])
        .output()
        .await;
    let Ok(output) = output else {
        return false;
    };
    if !output.status.success() {
        return false;
    }
    for id in String::from_utf8_lossy(&output.stdout).lines() {
        if !id.is_empty() && container_is_protected(id).await {
            return true;
        }
    }
    false
}

pub async fn denied_request(request: &HelperRequest) -> Option<OperationError> {
    match request {
        HelperRequest::DockerStart { container }
        | HelperRequest::DockerStop { container }
        | HelperRequest::DockerRestart { container } => {
            if container_is_protected(container).await {
                Some(denied())
            } else {
                None
            }
        }
        HelperRequest::DockerComposeStart { project }
        | HelperRequest::DockerComposeStop { project }
        | HelperRequest::DockerComposeRestart { project } => {
            if compose_project_is_protected(project).await {
                Some(denied())
            } else {
                None
            }
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn protected_label_is_stable() {
        assert_eq!(PROTECTED_LABEL, "com.argus.protected");
    }
}
