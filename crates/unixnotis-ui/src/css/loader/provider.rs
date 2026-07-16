//! CSS provider loading with explicit fallback outcomes

use std::fs;
use std::path::Path;

use tracing::warn;

use super::merge::merge_css_with_overrides;
use super::model::CssFileLoadResult;
use super::tokens::ensure_base_tokens;
use super::urls::rebase_relative_css_asset_urls;

/// Load CSS into a provider, applying overrides and falling back to defaults
pub fn load_provider_with_overrides(
    load_css_data: impl Fn(&str),
    path: &Path,
    fallback: &str,
    overrides: &str,
    inject_base_tokens: bool,
) -> CssFileLoadResult {
    match fs::read_to_string(path) {
        Ok(contents) => {
            let contents = if inject_base_tokens {
                ensure_base_tokens(&contents, path)
            } else {
                contents
            };
            if contents.trim().is_empty() {
                // Empty files fall back to embedded defaults so windows stay styled
                let merged = merge_css_with_overrides(fallback, fallback, overrides);
                // Relative url(...) assets break when CSS is loaded from raw bytes,
                // so rebase them against the stylesheet path before GTK sees the data
                load_css_data(&rebase_relative_css_asset_urls(&merged, path));
                return CssFileLoadResult::empty_fallback();
            }
            let merged = merge_css_with_overrides(&contents, fallback, overrides);
            // The provider still loads merged data, but the asset URLs now point at real files
            load_css_data(&rebase_relative_css_asset_urls(&merged, path));
            CssFileLoadResult::custom()
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
                // Fallback CSS can carry relative assets too, so it needs the same rebasing path
                load_css_data(&rebase_relative_css_asset_urls(&fallback, path));
                return CssFileLoadResult::read_failure(err.to_string());
            }
            let merged = format!("{fallback}\n{overrides}");
            // Overrides are merged before rebasing so later asset refs all see one final stylesheet
            load_css_data(&rebase_relative_css_asset_urls(&merged, path));
            CssFileLoadResult::read_failure(err.to_string())
        }
    }
}
