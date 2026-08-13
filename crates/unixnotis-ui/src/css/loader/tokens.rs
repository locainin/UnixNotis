//! Base CSS token compatibility helpers

use std::path::Path;

use tracing::warn;

pub(super) fn ensure_base_tokens(contents: &str, path: &Path) -> String {
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
        prefix = r"@define-color unixnotis-surface-base @unixnotis-surface;
@define-color unixnotis-surface-strong-base @unixnotis-surface-strong;
@define-color unixnotis-card-base @unixnotis-card;",
    )
}

#[cfg(test)]
#[path = "tests/tokens.rs"]
mod tests;
