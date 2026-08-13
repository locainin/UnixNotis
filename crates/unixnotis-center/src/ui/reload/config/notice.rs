//! Reload notice rendering and failure priority

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use gtk::prelude::*;
use unixnotis_core::css::hooks;
use unixnotis_ui::css::CssReloadReport;

use super::outcome::ReloadFailure;
use crate::ui::reload::notices::{ReloadNotice, ReloadNoticeFingerprint, ReloadNoticeKind};
use crate::ui::UiState;

impl UiState {
    pub(super) fn show_config_reload_failure(&mut self, failure: &ReloadFailure) {
        let detail = match failure {
            ReloadFailure::Config(error) => error.shareable_summary(),
            ReloadFailure::ThemeBase(detail) | ReloadFailure::ThemePaths(detail) => detail,
        };
        let detail = unixnotis_core::util::sanitize_inline_display_text(detail);
        let message =
            format!("Config reload rejected\nThe previous configuration is still active\n{detail}");
        let identity = failure.safe_fingerprint();
        self.set_reload_notice(ReloadNoticeKind::Config, &message, true, &identity);
    }

    pub(in crate::ui) fn apply_css_reload_notice(&mut self, report: &CssReloadReport) {
        // Intentional empty files are valid fallback requests and do not produce a notice
        let failures = report.read_failures().collect::<Vec<_>>();
        if failures.is_empty() {
            self.clear_reload_notice(ReloadNoticeKind::Css);
            return;
        }
        let first = failures[0];
        // File names are sufficient for the panel and avoid exposing full account paths
        let file = first
            .path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("CSS file");
        let suffix = if failures.len() == 1 {
            String::new()
        } else {
            format!(" and {} other layer(s)", failures.len() - 1)
        };
        let message = format!(
            "Theme fallback active\n{file}{suffix} could not be read; embedded styling is active"
        );
        let identity = css_failure_fingerprint(&failures);
        self.set_reload_notice(ReloadNoticeKind::Css, &message, false, &identity);
    }

    pub(in crate::ui) fn set_reload_notice(
        &mut self,
        kind: ReloadNoticeKind,
        message: &str,
        error: bool,
        identity: &str,
    ) {
        self.reload_notices.set(ReloadNotice {
            fingerprint: ReloadNoticeFingerprint {
                kind,
                identity: identity.to_string(),
            },
            message: message.to_string(),
            error,
        });
        self.render_reload_notice();
    }

    fn render_reload_notice(&self) {
        let Some(notice) = self.reload_notices.visible() else {
            self.panel.reload_notice.revealer.set_reveal_child(false);
            return;
        };
        self.panel.reload_notice.label.set_label(&notice.message);
        self.panel
            .reload_notice
            .shell
            .remove_css_class(hooks::panel_shell::RELOAD_NOTICE_ERROR);
        self.panel
            .reload_notice
            .shell
            .remove_css_class(hooks::panel_shell::RELOAD_NOTICE_WARNING);
        self.panel
            .reload_notice
            .shell
            .add_css_class(if notice.error {
                hooks::panel_shell::RELOAD_NOTICE_ERROR
            } else {
                hooks::panel_shell::RELOAD_NOTICE_WARNING
            });
        self.panel.reload_notice.close.set_visible(true);
        self.panel.reload_notice.revealer.set_reveal_child(true);
    }

    pub(in crate::ui) fn clear_reload_notice(&mut self, kind: ReloadNoticeKind) {
        self.reload_notices.clear(kind);
        self.render_reload_notice();
    }

    pub(super) fn capture_notice_dismissal(&mut self) {
        // The close button hides GTK immediately, then the next event records that dismissal
        if !self.panel.reload_notice.revealer.reveals_child()
            && self.reload_notices.visible().is_some()
        {
            self.reload_notices.dismiss_visible();
        }
    }
}

fn css_failure_fingerprint(failures: &[&unixnotis_ui::css::CssLayerReload]) -> String {
    // The UI message stays compact while the hash distinguishes changed files and read errors
    let mut hasher = DefaultHasher::new();
    for failure in failures {
        format!("{:?}", failure.layer).hash(&mut hasher);
        failure.path.hash(&mut hasher);
        failure.error.hash(&mut hasher);
    }
    format!("{:016x}", hasher.finish())
}
