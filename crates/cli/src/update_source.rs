use anyhow::{Context, Result, bail};
use cli::lifecycle;
use std::{fs, path::Path};

const UPDATE_BRANCH_STATE_FILE: &str = "update-branch";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpdateTarget {
    Version(String),
    Branch(String),
}

impl UpdateTarget {
    pub fn display(&self) -> String {
        match self {
            Self::Version(version) => format!("version '{version}'"),
            Self::Branch(branch) => format!("branch '{branch}'"),
        }
    }

    pub fn requested_value(&self) -> &str {
        match self {
            Self::Version(version) => version,
            Self::Branch(branch) => branch,
        }
    }

    pub fn env_pair(&self) -> (&'static str, &str) {
        match self {
            Self::Version(version) => ("ARGUS_TARGET_VERSION", version),
            Self::Branch(branch) => ("ARGUS_TARGET_BRANCH", branch),
        }
    }
}

fn saved_branch_from(state_dir: &Path) -> Result<Option<String>> {
    let path = state_dir.join(UPDATE_BRANCH_STATE_FILE);
    let value = match fs::read_to_string(&path) {
        Ok(value) => value,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(error).with_context(|| format!("read {}", path.display()));
        }
    };
    let branch = value.trim();
    if branch.is_empty() {
        bail!("saved update branch in {} is empty", path.display());
    }
    Ok(Some(branch.to_string()))
}

pub fn resolve(version: Option<String>, branch: Option<String>) -> Result<UpdateTarget> {
    let state_dir = lifecycle::env_path("ARGUS_STATE_DIR", lifecycle::DEFAULT_STATE_DIR);
    resolve_from_state_dir(version, branch, &state_dir)
}

fn resolve_from_state_dir(
    version: Option<String>,
    branch: Option<String>,
    state_dir: &Path,
) -> Result<UpdateTarget> {
    if version.is_some() && branch.is_some() {
        bail!("--version and --branch cannot be used together");
    }
    if let Some(branch) = branch {
        return Ok(UpdateTarget::Branch(branch));
    }
    if let Some(version) = version {
        return Ok(UpdateTarget::Version(version));
    }
    if let Some(branch) = saved_branch_from(state_dir)? {
        return Ok(UpdateTarget::Branch(branch));
    }
    Ok(UpdateTarget::Version("main".to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use uuid::Uuid;

    fn test_state_dir() -> std::path::PathBuf {
        std::env::temp_dir().join(format!("argus-update-source-{}", Uuid::new_v4()))
    }

    #[test]
    fn defaults_to_main_without_saved_branch() {
        let state_dir = test_state_dir();
        assert_eq!(
            resolve_from_state_dir(None, None, &state_dir).expect("resolve default target"),
            UpdateTarget::Version("main".to_string())
        );
    }

    #[test]
    fn saved_branch_becomes_default_update_source() {
        let state_dir = test_state_dir();
        fs::create_dir_all(&state_dir).expect("create state dir");
        fs::write(state_dir.join(UPDATE_BRANCH_STATE_FILE), "design/saasframe\n")
            .expect("write branch state");
        assert_eq!(
            resolve_from_state_dir(None, None, &state_dir).expect("resolve saved branch"),
            UpdateTarget::Branch("design/saasframe".to_string())
        );
        let _ = fs::remove_dir_all(state_dir);
    }

    #[test]
    fn explicit_version_overrides_saved_branch() {
        let state_dir = test_state_dir();
        fs::create_dir_all(&state_dir).expect("create state dir");
        fs::write(state_dir.join(UPDATE_BRANCH_STATE_FILE), "design/saasframe\n")
            .expect("write branch state");
        assert_eq!(
            resolve_from_state_dir(Some("main".to_string()), None, &state_dir)
                .expect("resolve explicit version"),
            UpdateTarget::Version("main".to_string())
        );
        let _ = fs::remove_dir_all(state_dir);
    }
}
