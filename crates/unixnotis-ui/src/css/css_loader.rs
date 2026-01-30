//! CSS file loading helpers with fallback and override handling.

use std::fs;
use std::path::Path;

use gtk::CssProvider;
use tracing::warn;

/// Load CSS into a provider, applying overrides and falling back to defaults.
pub(crate) fn load_provider_with_overrides(
    provider: &CssProvider,
    path: &Path,
    fallback: &str,
    overrides: &str,
    inject_base_tokens: bool,
) {
    match fs::read_to_string(path) {
        Ok(contents) => {
            let contents = if inject_base_tokens {
                ensure_base_tokens(&contents, path)
            } else {
                contents
            };
            if contents.trim().is_empty() {
                // Empty files fall back to embedded defaults so windows stay styled.
                let merged = if overrides.trim().is_empty() {
                    fallback.to_string()
                } else {
                    format!("{fallback}\n{overrides}")
                };
                provider.load_from_data(&merged);
                return;
            }
            let is_default = contents.trim() == fallback.trim();
            // User overrides should win when the file diverges from defaults.
            let merged = if overrides.trim().is_empty() {
                contents
            } else if is_default {
                format!("{contents}\n{overrides}")
            } else {
                format!("{overrides}\n{contents}")
            };
            provider.load_from_data(&merged);
        }
        Err(err) => {
            let file = path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("css");
            warn!(
                ?err,
                file, "failed to read css file; falling back to defaults"
            );
            let fallback = if inject_base_tokens {
                ensure_base_tokens(fallback, path)
            } else {
                fallback.to_string()
            };
            if overrides.trim().is_empty() {
                provider.load_from_data(&fallback);
                return;
            }
            let merged = format!("{fallback}\n{overrides}");
            provider.load_from_data(&merged);
        }
    }
}

pub(crate) fn ensure_base_tokens(contents: &str, path: &Path) -> String {
    if contents.contains("unixnotis-surface-base") && contents.contains("unixnotis-card-base") {
        return contents.to_string();
    }
    let file = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("base.css");
    warn!(
        file,
        "base css missing base color tokens; alpha overrides may be compounded until updated"
    );
    format!(
        "{prefix}\n{contents}",
        prefix = r#"@define-color unixnotis-surface-base @unixnotis-surface;
@define-color unixnotis-surface-strong-base @unixnotis-surface-strong;
@define-color unixnotis-card-base @unixnotis-card;"#,
    )
}
