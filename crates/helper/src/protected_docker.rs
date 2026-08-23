use helper::docker;
use protocol::{HelperRequest, OperationError};

fn denied() -> OperationError {
    OperationError {
        code: "PERMISSION_DENIED".into(),
        message: "Argus control-plane containers are protected from managed Docker actions".into(),
    }
}

fn protection_check_failed(error: impl std::fmt::Display) -> OperationError {
    OperationError {
        code: "DOCKER_PROTECTION_CHECK_FAILED".into(),
        message: format!("could not verify Docker protection labels: {error}"),
    }
}

async fn protected_result(result: Result<bool, helper::HelperError>) -> Option<OperationError> {
    match result {
        Ok(true) => Some(denied()),
        Ok(false) => None,
        Err(error) => Some(protection_check_failed(error)),
    }
}

pub async fn denied_request(request: &HelperRequest) -> Option<OperationError> {
    match request {
        HelperRequest::DockerStart { container }
        | HelperRequest::DockerStop { container }
        | HelperRequest::DockerRestart { container } => {
            protected_result(docker::container_is_protected(container).await).await
        }
        HelperRequest::DockerComposeStart { project }
        | HelperRequest::DockerComposeStop { project }
        | HelperRequest::DockerComposeRestart { project } => {
            if project.is_empty() || project.len() > 128 {
                return None;
            }
            protected_result(docker::compose_project_is_protected(project).await).await
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn protection_check_failure_is_fail_closed() {
        let error = protection_check_failed("daemon unavailable");
        assert_eq!(error.code, "DOCKER_PROTECTION_CHECK_FAILED");
    }
}
