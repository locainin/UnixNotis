//! Active theme containment checks for portable presets

use std::path::Path;

use anyhow::{anyhow, Result};

use crate::preset::pathing::normalize_lexical_path;

pub(in crate::preset::export) fn validate_theme_paths_stay_in_root(
    config_dir: &Path,
    theme_paths: &[(&'static str, &Path)],
) -> Result<()> {
    let normalized_root = normalize_lexical_path(config_dir);

    // A shareable preset must not depend on files stored outside the config root
    for (slot_name, path) in theme_paths {
        let normalized_path = normalize_lexical_path(path);
        if !normalized_path.starts_with(&normalized_root) {
            return Err(anyhow!(
                "preset export requires {} to live under the config root: {}",
                slot_name,
                path.display()
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "tests/theme.rs"]
mod tests;
