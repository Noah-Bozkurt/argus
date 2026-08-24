use crate::update_source::UpdateTarget;
use indicatif::{ProgressBar, ProgressDrawTarget, ProgressStyle};
use std::{io::IsTerminal, time::Duration};

struct ActiveProgress {
    bar: ProgressBar,
    message: String,
    detail: Option<String>,
    image_pull: bool,
}

impl ActiveProgress {
    fn render_message(&self) -> String {
        if self.image_pull {
            return self.detail.clone().unwrap_or_else(|| self.message.clone());
        }
        match &self.detail {
            Some(detail) => format!("{} · {detail}", self.message),
            None => self.message.clone(),
        }
    }

    fn refresh(&self) {
        self.bar.set_message(self.render_message());
    }
}

pub(crate) struct UpdateUi {
    color: bool,
    interactive: bool,
    post_start: bool,
    download_announced: bool,
    rollback_started: bool,
    rollback_completed: bool,
    finished: bool,
    progress: Option<ActiveProgress>,
}

impl UpdateUi {
    pub(crate) fn new() -> Self {
        let interactive = std::io::stdout().is_terminal();
        Self {
            color: interactive && std::env::var_os("NO_COLOR").is_none(),
            interactive,
            post_start: false,
            download_announced: false,
            rollback_started: false,
            rollback_completed: false,
            finished: false,
            progress: None,
        }
    }

    fn spinner_style() -> ProgressStyle {
        ProgressStyle::with_template("  {spinner:.cyan} {msg} · {elapsed_precise}")
            .expect("valid Argus update spinner template")
            .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"])
    }

    fn bar_style() -> ProgressStyle {
        ProgressStyle::with_template("  [{bar:24.cyan}] {msg} · {percent:>3}% · {elapsed_precise}")
            .expect("valid Argus update progress template")
            .progress_chars("█░")
    }

    fn paint(&self, code: &str, text: &str) -> String {
        if self.color {
            format!("\x1b[{code}m{text}\x1b[0m")
        } else {
            text.to_string()
        }
    }

    pub(crate) fn begin(&self, target: &UpdateTarget) {
        println!("{}", self.paint("1;36", "Argus update"));
        self.detail(&format!("Source: {}", target.display()));
        println!();
    }

    fn step(&self, message: &str) {
        println!("{} {message}", self.paint("36", "  ›"));
    }

    fn success(&self, message: &str) {
        println!("{} {message}", self.paint("32", "  ✓"));
    }

    fn warning(&self, message: &str) {
        eprintln!("{} {message}", self.paint("33", "  !"));
    }

    fn error(&self, message: &str) {
        eprintln!("{} {message}", self.paint("31", "  ✗"));
    }

    fn detail(&self, message: &str) {
        println!("{}", self.paint("2", &format!("    {message}")));
    }

    pub(crate) fn short_revision(revision: &str) -> &str {
        revision.get(..12).unwrap_or(revision)
    }

    pub(crate) fn stop_progress(&mut self) {
        if let Some(progress) = self.progress.take() {
            progress.bar.finish_and_clear();
        }
    }

    fn start_progress(&mut self, message: impl Into<String>) {
        self.stop_progress();
        let message = message.into();
        if !self.interactive {
            self.step(&message);
            return;
        }
        let bar = ProgressBar::new_spinner();
        bar.set_draw_target(ProgressDrawTarget::stdout());
        bar.set_style(Self::spinner_style());
        bar.set_message(message.clone());
        bar.enable_steady_tick(Duration::from_millis(100));
        self.progress = Some(ActiveProgress {
            bar,
            message,
            detail: None,
            image_pull: false,
        });
    }

    fn set_progress_detail(&mut self, detail: impl Into<String>) {
        if let Some(progress) = self.progress.as_mut() {
            progress.detail = Some(detail.into());
            progress.refresh();
        }
    }

    fn set_image_pull(&mut self, image_pull: bool) {
        if let Some(progress) = self.progress.as_mut() {
            progress.image_pull = image_pull;
            progress.refresh();
        }
    }

    fn set_progress_value(&mut self, current: u64, total: u64) {
        if let Some(progress) = self.progress.as_mut()
            && total > 0
        {
            progress.bar.set_length(total);
            progress.bar.set_position(current.min(total));
            progress.bar.set_style(Self::bar_style());
            progress.refresh();
        }
    }

    fn finish_progress_line(&mut self) {
        if let Some(progress) = self.progress.take() {
            progress.bar.finish_and_clear();
        }
    }

    pub(crate) fn handle_line(&mut self, line: &str) {
        let line = line.trim();
        if line.is_empty() {
            return;
        }

        if let Some(payload) = line.strip_prefix("[argus-pull-progress]\t") {
            let mut fields = payload.splitn(4, '\t');
            let image = fields.next().unwrap_or_default();
            let current = fields.next().and_then(|value| value.parse::<u64>().ok());
            let total = fields.next().and_then(|value| value.parse::<u64>().ok());
            let status = fields.next().unwrap_or_default();
            if let (Some(current), Some(total)) = (current, total) {
                self.set_progress_value(current, total);
            }
            if !image.is_empty() {
                let image = cli::progress::short_image_name(image).to_string();
                let complete = status.trim().eq_ignore_ascii_case("complete");
                self.set_image_pull(true);
                self.set_progress_detail(if complete {
                    format!("{image} · ✓")
                } else {
                    image.clone()
                });
                if complete {
                    self.finish_progress_line();
                    self.success(&format!("Downloaded {image}"));
                }
            }
            return;
        }

        if let Some(revision) = line.strip_prefix("[argus-update] current installed revision: ") {
            self.stop_progress();
            self.step(&format!(
                "Checking current installation ({})",
                Self::short_revision(revision)
            ));
            return;
        }
        if line == "[argus-update] verifying current installation before update" {
            return;
        }
        if line.starts_with("Argus first-server smoke test passed:") {
            self.stop_progress();
            if self.post_start {
                self.success("Updated installation is healthy");
            } else {
                self.success("Current installation is healthy");
            }
            return;
        }
        if line.starts_with("[argus-update] resolving branch '") {
            self.start_progress("Downloading branch source");
            return;
        }
        if line.starts_with("[argus-update] building branch '") {
            self.start_progress("Building branch locally");
            self.set_progress_detail("Docker BuildKit");
            return;
        }
        if line.starts_with("[argus-update] using prepared branch build ") {
            self.stop_progress();
            self.step("Using verified local branch build");
            return;
        }
        if line.starts_with("[argus-update] resolving target '") {
            self.start_progress("Resolving update target");
            return;
        }
        if let Some(image) = line.strip_prefix("[argus-update] pre-fetching ") {
            if !self.download_announced {
                self.download_announced = true;
                self.start_progress("Downloading update");
            } else if self.progress.is_none() {
                self.start_progress("Downloading update");
            }
            let image = image
                .rsplit('/')
                .next()
                .unwrap_or(image)
                .split(':')
                .next()
                .unwrap_or(image);
            self.set_progress_detail(format!("pulling {image}"));
            return;
        }
        if let Some(revision) = line.strip_prefix("[argus-update] resolved target revision: ") {
            self.stop_progress();
            self.success(&format!(
                "Update ready ({})",
                Self::short_revision(revision)
            ));
            return;
        }
        if let Some(revision) =
            line.strip_prefix("[argus-update] already running requested revision ")
        {
            self.stop_progress();
            self.success(&format!(
                "Already up to date ({})",
                Self::short_revision(revision)
            ));
            self.finished = true;
            return;
        }
        if line.starts_with("[argus-update] storage preflight: ") {
            self.stop_progress();
            self.step("Checking backup storage");
            return;
        }
        if line == "[argus-update] quiescing native Agent/Helper and control-plane writers" {
            self.stop_progress();
            self.success("Backup storage is sufficient");
            self.step("Stopping Argus services");
            return;
        }
        if line == "[argus-update] creating consistent PostgreSQL backup" {
            self.start_progress("Creating rollback backup");
            return;
        }
        if line == "[argus-update] installing target deployment assets and native binaries" {
            self.start_progress("Installing update");
            return;
        }
        if let Some(revision) = line.strip_prefix("[argus-update] starting target control plane ") {
            self.post_start = true;
            self.start_progress(format!("Starting Argus {}", Self::short_revision(revision)));
            return;
        }
        if self.post_start && line == "[argus-smoke] validating deployed configuration" {
            self.start_progress("Verifying updated installation");
            return;
        }
        if let Some(change) = line.strip_prefix("[argus-update] update succeeded: ") {
            self.stop_progress();
            let change = change
                .split(" -> ")
                .map(Self::short_revision)
                .collect::<Vec<_>>()
                .join(" → ");
            println!();
            self.success(&format!("Update complete: {change}"));
            self.finished = true;
            return;
        }
        if let Some(path) = line.strip_prefix("[argus-update] rollback snapshot retained at ") {
            self.stop_progress();
            self.detail(&format!("Rollback snapshot: {path}"));
            return;
        }
        if let Some(message) = line.strip_prefix("[argus-update] warning: ") {
            self.stop_progress();
            if message.starts_with("update failed; automatically rolling back transaction ") {
                self.rollback_started = true;
                println!();
                self.warning("Update failed; restoring the previous version");
                self.start_progress("Restoring previous version");
            } else if message.starts_with("rollback completed successfully; restored revision ") {
                self.rollback_completed = true;
                let revision = message
                    .trim_start_matches("rollback completed successfully; restored revision ");
                self.success(&format!(
                    "Rollback completed ({})",
                    Self::short_revision(revision)
                ));
            } else {
                self.warning(message);
            }
            return;
        }
        if let Some(message) = line.strip_prefix("[argus-update] error: ") {
            self.stop_progress();
            self.error(message);
            return;
        }
        if let Some(message) = line.strip_prefix("[argus-smoke] FAIL: ") {
            self.stop_progress();
            self.error(message);
            return;
        }

        let lower = line.to_ascii_lowercase();
        if lower.starts_with("error:")
            || lower.starts_with("error response from daemon")
            || lower.contains("permission denied")
        {
            self.stop_progress();
            self.error(line);
        }
    }

    pub(crate) fn finish_failure(&mut self) {
        self.stop_progress();
        if !self.finished {
            println!();
            if self.rollback_completed {
                self.error("Update did not complete; the previous version was restored");
            } else if self.rollback_started {
                self.error("Update and automatic rollback did not complete cleanly");
            } else {
                self.error("Update failed");
            }
            self.detail("Re-run with --verbose for full diagnostics");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn update_ui_shortens_full_revisions() {
        assert_eq!(
            UpdateUi::short_revision("0123456789abcdef0123456789abcdef01234567"),
            "0123456789ab"
        );
        assert_eq!(UpdateUi::short_revision("main"), "main");
    }

    #[test]
    fn progress_styles_are_valid() {
        let _ = UpdateUi::spinner_style();
        let _ = UpdateUi::bar_style();
    }
}
