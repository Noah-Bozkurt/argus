from pathlib import Path


def replace(path: str, old: str, new: str) -> None:
    p = Path(path)
    text = p.read_text()
    if old not in text:
        raise SystemExit(f"missing expected block in {path}: {old[:80]!r}")
    p.write_text(text.replace(old, new, 1))


replace(
    "crates/cli/src/lib.rs",
    "pub mod domain;\npub mod lifecycle;\n",
    "pub mod docker;\npub mod domain;\npub mod lifecycle;\n",
)

replace(
    "crates/cli/src/main.rs",
    "use cli::{domain, lifecycle};",
    "use cli::{docker, domain, lifecycle};",
)
replace(
    "crates/cli/src/main.rs",
    "    #[command(hide = true)]\n    RecoverUpdate {\n        #[arg(long)]\n        retry_failed: bool,\n    },\n    /// Display local host information.",
    "    #[command(hide = true)]\n    RecoverUpdate {\n        #[arg(long)]\n        retry_failed: bool,\n    },\n    #[command(hide = true)]\n    DockerPull {\n        image: String,\n    },\n    /// Display local host information.",
)
replace(
    "crates/cli/src/main.rs",
    "struct ActiveProgress {\n    message: String,\n    detail: Option<String>,\n    started: Instant,\n}",
    "struct ActiveProgress {\n    message: String,\n    detail: Option<String>,\n    current: Option<u64>,\n    total: Option<u64>,\n    started: Instant,\n}",
)
replace(
    "crates/cli/src/main.rs",
    "    fn elapsed_label(seconds: u64) -> String {",
    "    fn determinate_bar(current: u64, total: u64) -> String {\n        if total == 0 {\n            return Self::indeterminate_bar(0);\n        }\n        let filled = ((current.min(total) as u128 * UPDATE_PROGRESS_WIDTH as u128)\n            / total as u128) as usize;\n        let mut cells = vec!['░'; UPDATE_PROGRESS_WIDTH];\n        for cell in cells.iter_mut().take(filled) {\n            *cell = '█';\n        }\n        cells.into_iter().collect()\n    }\n\n    fn elapsed_label(seconds: u64) -> String {",
)
replace(
    "crates/cli/src/main.rs",
    "        self.progress = Some(ActiveProgress {\n            message,\n            detail: None,\n            started: Instant::now(),\n        });",
    "        self.progress = Some(ActiveProgress {\n            message,\n            detail: None,\n            current: None,\n            total: None,\n            started: Instant::now(),\n        });",
)
replace(
    "crates/cli/src/main.rs",
    "    fn set_progress_detail(&mut self, detail: impl Into<String>) {\n        if let Some(progress) = self.progress.as_mut() {\n            progress.detail = Some(detail.into());\n        }\n    }\n\n    fn tick(&mut self) {",
    "    fn set_progress_detail(&mut self, detail: impl Into<String>) {\n        if let Some(progress) = self.progress.as_mut() {\n            progress.detail = Some(detail.into());\n        }\n    }\n\n    fn set_progress_value(&mut self, current: u64, total: u64) {\n        if let Some(progress) = self.progress.as_mut()\n            && total > 0\n        {\n            progress.current = Some(current.min(total));\n            progress.total = Some(total);\n        }\n    }\n\n    fn tick(&mut self) {",
)
replace(
    "crates/cli/src/main.rs",
    "        let elapsed = progress.started.elapsed().as_secs();\n        let bar = Self::indeterminate_bar(self.progress_frame);\n        self.progress_frame = self.progress_frame.wrapping_add(1);\n        let bar = self.paint(\"36\", &format!(\"[{bar}]\"));\n        let activity = if elapsed >= 15 {",
    "        let elapsed = progress.started.elapsed().as_secs();\n        let (bar, percentage) = match (progress.current, progress.total) {\n            (Some(current), Some(total)) if total > 0 => {\n                let percent = (current.min(total) as u128 * 100 / total as u128) as u64;\n                (Self::determinate_bar(current, total), format!(\" · {percent:>3}%\"))\n            }\n            _ => {\n                let bar = Self::indeterminate_bar(self.progress_frame);\n                self.progress_frame = self.progress_frame.wrapping_add(1);\n                (bar, String::new())\n            }\n        };\n        let bar = self.paint(\"36\", &format!(\"[{bar}]\"));\n        let activity = if elapsed >= 15 {",
)
replace(
    "crates/cli/src/main.rs",
    "            \"\\r\\x1b[2K  {bar} {message}{detail} · {}{activity}\",\n            Self::elapsed_label(elapsed)",
    "            \"\\r\\x1b[2K  {bar} {message}{detail}{percentage} · {}{activity}\",\n            Self::elapsed_label(elapsed)",
)
replace(
    "crates/cli/src/main.rs",
    "        if let Some(revision) = line.strip_prefix(\"[argus-update] current installed revision: \") {",
    "        if let Some(payload) = line.strip_prefix(\"[argus-pull-progress]\\t\") {\n            let mut fields = payload.splitn(4, '\\t');\n            let image = fields.next().unwrap_or_default();\n            let current = fields.next().and_then(|value| value.parse::<u64>().ok());\n            let total = fields.next().and_then(|value| value.parse::<u64>().ok());\n            let status = fields.next().unwrap_or_default();\n            if let (Some(current), Some(total)) = (current, total) {\n                self.set_progress_value(current, total);\n            }\n            if !image.is_empty() {\n                let image = docker::short_image_name(image);\n                let status = status.trim();\n                if status.is_empty() {\n                    self.set_progress_detail(image.to_string());\n                } else {\n                    self.set_progress_detail(format!(\"{image} · {}\", status.to_ascii_lowercase()));\n                }\n            }\n            return;\n        }\n\n        if let Some(revision) = line.strip_prefix(\"[argus-update] current installed revision: \") {",
)
replace(
    "crates/cli/src/main.rs",
    "        .env(env_key, env_value)\n        // The embedded updater has a compatibility spinner for direct shell use.",
    "        .env(env_key, env_value)\n        .env(\"ARGUS_UPDATE_BOLLARD_PULL\", \"1\")\n        // The embedded updater has a compatibility spinner for direct shell use.",
)
replace(
    "crates/cli/src/main.rs",
    "        Commands::Uninstall { yes, purge_data } => run_uninstall(yes, purge_data).await?,\n        Commands::Domain { command } => match command {",
    "        Commands::Uninstall { yes, purge_data } => run_uninstall(yes, purge_data).await?,\n        Commands::DockerPull { image } => {\n            docker::pull_image(&image, |progress| {\n                let status = progress.status.replace('\\t', \" \").replace('\\n', \" \" );\n                println!(\n                    \"[argus-pull-progress]\\t{}\\t{}\\t{}\\t{}\",\n                    progress.image, progress.current, progress.total, status\n                );\n            })\n            .await?;\n        }\n        Commands::Domain { command } => match command {",
)

replace(
    "scripts/update-first-test.sh",
    "pull_image() {\n  local ref=\"$1\" output status summary\n\n  if [[ \"${ARGUS_UPDATE_VERBOSE:-0}\" == \"1\" ]] || [[ -t 1 ]]; then",
    "pull_image() {\n  local ref=\"$1\" output status summary helper\n\n  if [[ \"${ARGUS_UPDATE_BOLLARD_PULL:-0}\" == \"1\" ]]; then\n    helper=\"${ARGUS_UPDATE_PULL_HELPER:-/proc/$PPID/exe}\"\n    [[ -x \"$helper\" ]] || die \"Bollard pull helper is unavailable: $helper\"\n    set +e\n    \"$helper\" docker-pull \"$ref\"\n    status=$?\n    set -e\n    (( status == 0 )) || die \"failed to pull $ref through Docker Engine API (exit $status)\"\n    return\n  fi\n\n  if [[ \"${ARGUS_UPDATE_VERBOSE:-0}\" == \"1\" ]] || [[ -t 1 ]]; then",
)

replace(
    "crates/cli/src/installer_shared.rs",
    "    thread,\n    time::Duration,",
    "    thread,\n    time::{Duration, Instant},",
)
replace(
    "crates/cli/src/installer_shared.rs",
    "fn elapsed_label(seconds: u64) -> String {",
    "fn determinate_bar(current: u64, total: u64) -> String {\n    if total == 0 {\n        return indeterminate_bar(0);\n    }\n    let filled = ((current.min(total) as u128 * PROGRESS_WIDTH as u128) / total as u128) as usize;\n    let mut cells = vec!['░'; PROGRESS_WIDTH];\n    for cell in cells.iter_mut().take(filled) {\n        *cell = '█';\n    }\n    cells.into_iter().collect()\n}\n\nfn elapsed_label(seconds: u64) -> String {",
)
replace(
    "crates/cli/src/installer_shared.rs",
    "    pub(crate) fn working<T>(&self, message: &str, work: impl FnOnce() -> Result<T>) -> Result<T> {",
    "    pub(crate) fn pull_images(&self, images: &[String]) -> Result<()> {\n        let message = \"Downloading control-plane images\";\n        self.record(&format!(\"START: {message}\"));\n        let interactive = !self.verbose && std::io::stdout().is_terminal();\n        let started = Instant::now();\n        if interactive {\n            let bar = self.paint(\"36\", &format!(\"[{}]\", indeterminate_bar(0)));\n            print!(\"\\r\\x1b[2K  {bar} {message} · connecting\");\n            let _ = std::io::stdout().flush();\n        } else {\n            println!(\"{} {message}\", self.paint(\"36\", \"  ›\"));\n        }\n\n        let result = lifecycle::docker_pull_images(images, |progress| {\n            if !interactive {\n                return;\n            }\n            let (bar, percentage) = if progress.total > 0 {\n                let percent = (progress.current.min(progress.total) as u128 * 100\n                    / progress.total as u128) as u64;\n                (\n                    determinate_bar(progress.current, progress.total),\n                    format!(\" · {percent:>3}%\"),\n                )\n            } else {\n                (indeterminate_bar(0), String::new())\n            };\n            let bar = self.paint(\"36\", &format!(\"[{bar}]\"));\n            let image = lifecycle::docker_short_image_name(&progress.image);\n            let status = progress.status.to_ascii_lowercase();\n            print!(\n                \"\\r\\x1b[2K  {bar} {message} · {image} · {status}{percentage} · {}\",\n                elapsed_label(started.elapsed().as_secs())\n            );\n            let _ = std::io::stdout().flush();\n        });\n\n        if interactive {\n            print!(\"\\r\\x1b[2K\");\n            let _ = std::io::stdout().flush();\n        }\n        if result.is_ok() {\n            println!(\"{} {message}\", self.paint(\"32\", \"  ✓\"));\n            self.record(&format!(\"OK: {message}\"));\n        } else if let Err(error) = &result {\n            self.record(&format!(\"FAILED: {message}: {error:#}\"));\n        }\n        result\n    }\n\n    pub(crate) fn working<T>(&self, message: &str, work: impl FnOnce() -> Result<T>) -> Result<T> {",
)

replace(
    "crates/cli/src/installer_control.rs",
    "    pub(crate) fn start_control_plane(&self) -> Result<()> {\n        // Rendered Compose configuration contains credentials. Validation must stay\n        // quiet even when verbose diagnostics are enabled.\n        self.compose_status(&[\"config\", \"--quiet\"])?;\n        self.compose_status(&[\"pull\"])?;\n        self.configure_firewall_if_active()?;",
    "    pub(crate) fn pull_control_plane_images(&self) -> Result<()> {\n        // Rendered Compose configuration contains credentials. Validation must stay\n        // quiet even when verbose diagnostics are enabled.\n        self.compose_status(&[\"config\", \"--quiet\"])?;\n        let images = self.compose_output(&[\"config\", \"--images\"])?;\n        let images = images\n            .lines()\n            .map(str::trim)\n            .filter(|image| !image.is_empty())\n            .map(ToOwned::to_owned)\n            .collect::<Vec<_>>();\n        if images.is_empty() {\n            bail!(\"Compose configuration did not resolve any images\");\n        }\n        self.ui.pull_images(&images)\n    }\n\n    pub(crate) fn start_control_plane(&self) -> Result<()> {\n        self.configure_firewall_if_active()?;",
)

replace(
    "crates/cli/src/installer.rs",
    "            self.ui.working(\"Configuring Argus services\", || {\n                self.ensure_argus_user()?;\n                self.provision_tls(&config)?;\n                self.write_runtime_env(&config)?;\n                self.generate_caddy_config(&config)\n            })?;\n            self.ui.working(\"Starting the control plane\", || {",
    "            self.ui.working(\"Configuring Argus services\", || {\n                self.ensure_argus_user()?;\n                self.provision_tls(&config)?;\n                self.write_runtime_env(&config)\n            })?;\n            self.pull_control_plane_images()?;\n            self.ui.working(\"Configuring the reverse proxy\", || {\n                self.generate_caddy_config(&config)\n            })?;\n            self.ui.working(\"Starting the control plane\", || {",
)

# Keep Docker/Bollard access behind lifecycle wrappers for the installer UI.
lifecycle = Path("crates/cli/src/lifecycle.rs")
text = lifecycle.read_text()
insert = """

pub fn docker_pull_images<F>(images: &[String], report: F) -> Result<()>
where
    F: FnMut(crate::docker::PullProgress),
{
    crate::docker::pull_images_blocking(images, report)
}

pub fn docker_short_image_name(image: &str) -> &str {
    crate::docker::short_image_name(image)
}
"""
marker = "\npub fn registry_config() -> RegistryConfig {"
if insert.strip() not in text:
    if marker not in text:
        raise SystemExit("missing lifecycle registry_config marker")
    text = text.replace(marker, insert + marker, 1)
    lifecycle.write_text(text)
