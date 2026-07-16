//! Log-only CSS reload diagnostics for the popup process

use unixnotis_ui::css::CssReloadReport;

pub fn log_reload_failures(report: &CssReloadReport, context: &str) {
    // Popup reload failures stay log-only so transient errors never steal focus
    for failure in report.read_failures() {
        // Sanitization removes terminal controls before structured logging
        let detail = failure.error.as_deref().unwrap_or("CSS file read failed");
        let detail = unixnotis_core::util::sanitize_inline_display_text(detail);
        tracing::warn!(
            layer = ?failure.layer,
            path = %failure.path.display(),
            %context,
            error = %detail,
            "popup CSS reload used embedded fallback"
        );
    }
}

#[cfg(test)]
#[path = "tests/css_reload.rs"]
mod tests;
