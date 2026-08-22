use anyhow::{Result, bail};
use cli::lifecycle;
use std::path::Path;

const UPDATE_BRANCH_ENV: &str = "ARGUS_UPDATE_BRANCH";

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

    pub fn env_pair(&self) -> (&'static str, &str) {
        match self {
            Self::Version(version) => ("ARGUS_TARGET_VERSION", version),
            Self::Branch(branch) => ("ARGUS_TARGET_BRANCH", branch),
        }
    }
}

fn saved_branch_from(install_dir: &Path) -> Result<Option<String>> {
    let path = install_dir.join(".env");
    if !path.exists() {
        return Ok(None);
    }
    let values = lifecycle::read_env_file(&path)?;
    let Some(value) = values.get(UPDATE_BRANCH_ENV) else {
        return Ok(None);
    };
    let branch = value.trim();
    if branch.is_empty() {
        return Ok(None);
    }
    Ok(Some(branch.to_string()))
}

pub fn resolve(version: Option<String>, branch: Option<String>) -> Result<UpdateTarget> {
    let install_dir = lifecycle::env_path("ARGUS_INSTALL_DIR", lifecycle::DEFAULT_INSTALL_DIR);
    resolve_from_install_dir(version, branch, &install_dir)
}

fn resolve_from_install_dir(
    version: Option<String>,
    branch: Option<String>,
    install_dir: &Path,
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
    if let Some(branch) = saved_branch_from(install_dir)? {
        return Ok(UpdateTarget::Branch(branch));
    }
    Ok(UpdateTarget::Version("main".to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use uuid::Uuid;

    fn test_install_dir() -> std::path::PathBuf {
        std::env::temp_dir().join(format!("argus-update-source-{}", Uuid::new_v4()))
    }

    #[test]
    fn defaults_to_main_without_saved_branch() {
        let install_dir = test_install_dir();
        assert_eq!(
            resolve_from_install_dir(None, None, &install_dir).expect("resolve default target"),
            UpdateTarget::Version("main".to_string())
        );
    }

    #[test]
    fn saved_branch_becomes_default_update_source() {
        let install_dir = test_install_dir();
        fs::create_dir_all(&install_dir).expect("create install dir");
        fs::write(
            install_dir.join(".env"),
            "ARGUS_VERSION=abc\nARGUS_UPDATE_BRANCH=design/saasframe\n",
        )
        .expect("write installed env");
        assert_eq!(
            resolve_from_install_dir(None, None, &install_dir).expect("resolve saved branch"),
            UpdateTarget::Branch("design/saasframe".to_string())
        );
        let _ = fs::remove_dir_all(install_dir);
    }

    #[test]
    fn explicit_version_overrides_saved_branch() {
        let install_dir = test_install_dir();
        fs::create_dir_all(&install_dir).expect("create install dir");
        fs::write(
            install_dir.join(".env"),
            "ARGUS_UPDATE_BRANCH=design/saasframe\n",
        )
        .expect("write installed env");
        assert_eq!(
            resolve_from_install_dir(Some("main".to_string()), None, &install_dir)
                .expect("resolve explicit version"),
            UpdateTarget::Version("main".to_string())
        );
        let _ = fs::remove_dir_all(install_dir);
    }

    #[test]
    fn empty_saved_branch_falls_back_to_main() {
        let install_dir = test_install_dir();
        fs::create_dir_all(&install_dir).expect("create install dir");
        fs::write(install_dir.join(".env"), "ARGUS_UPDATE_BRANCH=\n")
            .expect("write installed env");
        assert_eq!(
            resolve_from_install_dir(None, None, &install_dir).expect("resolve default target"),
            UpdateTarget::Version("main".to_string())
        );
        let _ = fs::remove_dir_all(install_dir);
    }
}
