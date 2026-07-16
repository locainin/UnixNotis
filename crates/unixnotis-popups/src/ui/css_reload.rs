//! Log-only CSS reload diagnostics for the popup process

use unixnotis_ui::css::CssReloadReport;

pub fn log_reload_failures(report: &CssReloadReport, context: &str) {
    // Popup reload failures stay log-only so transient errors never steal focus
    for failure in report.read_failures() {
        // A filename identifies the configured layer without disclosing the account path
        let file = failure
            .path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("CSS file");
        tracing::warn!(
            layer = ?failure.layer,
            %file,
            %context,
            read_error = failure.error.is_some(),
            "popup CSS reload used embedded fallback"
        );
    }
}

#[cfg(test)]
#[path = "tests/css_reload.rs"]
mod tests;
