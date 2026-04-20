use std::path::{Path, PathBuf};

use super::super::main_css_check_files::format_display_path;
use super::super::main_css_check_report::{CssCheckActiveFile, CssCheckDiagnostic};

pub(in super::super) struct CssCheckInputs {
    // These are the real files that move into parse and lint later
    pub(in super::super) files: Vec<PathBuf>,
    // Active files are shown even when some files collapse onto one parse target
    pub(in super::super) active_files: Vec<CssCheckActiveFile>,
    pub(in super::super) notes: Vec<String>,
    pub(in super::super) diagnostics: Vec<CssCheckDiagnostic>,
}

pub(in super::super) struct ThemeTarget {
    pub(in super::super) slot_name: &'static str,
    pub(in super::super) config_key: &'static str,
    pub(in super::super) path: PathBuf,
}

impl ThemeTarget {
    pub(in super::super) fn display_path(&self, config_dir: &Path, display_root: &str) -> String {
        // Display rendering stays here so every caller formats targets the same way
        format_display_path(config_dir, display_root, &self.path)
    }
}
