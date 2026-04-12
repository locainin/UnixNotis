//! Export-only validation helpers
//!
//! These checks are specific to building a shareable preset from the live tree

use anyhow::{anyhow, Result};
use std::path::Path;

use crate::preset::pathing::normalize_lexical_path;

pub(super) fn validate_theme_paths_stay_in_root(
    config_dir: &Path,
    theme_paths: &[(&'static str, &Path)],
) -> Result<()> {
    let normalized_root = normalize_lexical_path(config_dir);

    // A shareable preset should not depend on files stored outside the config root
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
