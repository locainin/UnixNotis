//! Hyprland config path resolution

use std::env;
use std::path::PathBuf;

use anyhow::Result;

use crate::paths::home_dir;

pub(in crate::actions::hyprland) fn hyprland_config_path() -> Result<PathBuf> {
    // Respect XDG_CONFIG_HOME when it is set so custom config roots still work
    if let Ok(base) = env::var("XDG_CONFIG_HOME") {
        if !base.trim().is_empty() {
            return Ok(PathBuf::from(base).join("hypr").join("hyprland.conf"));
        }
    }

    // Fall back to the conventional ~/.config path when XDG is unset
    Ok(home_dir()?
        .join(".config")
        .join("hypr")
        .join("hyprland.conf"))
}
