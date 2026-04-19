use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use unixnotis_core::ThemePaths;

use super::model::ThemeTarget;

pub(in super::super) fn theme_targets(theme_paths: ThemePaths) -> [ThemeTarget; 5] {
    // Slot order stays fixed so reports remain stable between runs
    [
        ThemeTarget {
            slot_name: "base css",
            config_key: "[theme].base_css",
            path: theme_paths.base_css,
        },
        ThemeTarget {
            slot_name: "panel css",
            config_key: "[theme].panel_css",
            path: theme_paths.panel_css,
        },
        ThemeTarget {
            slot_name: "popup css",
            config_key: "[theme].popup_css",
            path: theme_paths.popup_css,
        },
        ThemeTarget {
            slot_name: "widgets css",
            config_key: "[theme].widgets_css",
            path: theme_paths.widgets_css,
        },
        ThemeTarget {
            slot_name: "media css",
            config_key: "[theme].media_css",
            path: theme_paths.media_css,
        },
    ]
}

pub(in super::super) fn has_css_extension(path: &Path) -> bool {
    // Case-insensitive matching keeps css-check aligned with common filesystem behavior
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.eq_ignore_ascii_case("css"))
        .unwrap_or(false)
}

pub(in super::super) fn normalize_target_key(path: &Path) -> PathBuf {
    // Duplicate-slot warnings should follow the real file when possible
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

pub(in super::super) fn dedupe_key_for_theme_file(path: &Path) -> PathBuf {
    // Real-file dedupe should follow symlinks when that works
    //
    // Falling back to the configured path keeps css-check usable even when
    // canonicalize cannot read every directory in the path yet
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

pub(in super::super) fn join_config_keys(keys: &[&'static str]) -> String {
    // Sorting through a set keeps the warning order stable and duplicate-free
    keys.iter()
        .copied()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>()
        .join(", ")
}
