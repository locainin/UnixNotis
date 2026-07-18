use std::path::Path;

use anyhow::Result;
use unixnotis_core::{Config, PANEL_RUNTIME_WIDTH_MIN};

use super::report::{CssCheckCategory, CssCheckDiagnostic};

pub(super) fn lint_runtime_config(
    config_dir: &Path,
    display_root: &str,
    config_path: &Path,
    config: &Config,
) -> Result<Vec<CssCheckDiagnostic>> {
    let mut diagnostics = Vec::new();

    if let Some(message) = panel_width_floor_warning(config) {
        // Runtime warnings should point at config.toml instead of a css file
        diagnostics.push(CssCheckDiagnostic::warning(
            CssCheckCategory::Runtime,
            display_config_path(config_dir, display_root, config_path),
            message,
        ));
    }

    Ok(diagnostics)
}

pub(super) fn display_config_path(
    config_dir: &Path,
    display_root: &str,
    config_path: &Path,
) -> String {
    // Keep config.toml paths in the same display style as css warnings
    config_path.strip_prefix(config_dir).map_or_else(
        |_| config_path.display().to_string(),
        |path| format!("{display_root}/{}", path.display()),
    )
}

pub(super) fn panel_width_floor_warning(config: &Config) -> Option<String> {
    if config.panel.width >= PANEL_RUNTIME_WIDTH_MIN {
        return None;
    }

    // Small panel widths can look ignored because the center clamps them later
    Some(format!(
        "[panel].width={} is below the runtime floor of {}; unixnotis-center will clamp it and the panel may look wider than config or css edits suggest",
        config.panel.width, PANEL_RUNTIME_WIDTH_MIN
    ))
}
