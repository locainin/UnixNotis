//! Hyprland config path resolution

use std::env;
use std::path::{Path, PathBuf};

use anyhow::Result;

use crate::paths::home_dir;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::actions::hyprland) enum HyprlandConfigSyntax {
    // Current Hyprland configs use Lua and need hl.exec_cmd startup calls
    Lua,
    // Legacy configs use hyprlang and still need exec-once lines
    Hyprlang,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::actions::hyprland) struct HyprlandConfigTarget {
    pub(in crate::actions::hyprland) path: PathBuf,
    pub(in crate::actions::hyprland) syntax: HyprlandConfigSyntax,
}

pub(in crate::actions::hyprland) fn hyprland_config_target() -> Result<HyprlandConfigTarget> {
    let config_home = hyprland_config_home()?;
    // Install updates only the config Hyprland will actually load at startup
    Ok(hyprland_config_target_in(&config_home))
}

pub(in crate::actions::hyprland) fn existing_hyprland_config_targets(
) -> Result<Vec<HyprlandConfigTarget>> {
    let config_home = hyprland_config_home()?;
    // Uninstall cleanup scans all known formats because users can migrate between releases
    Ok(existing_hyprland_config_targets_in(&config_home))
}

fn hyprland_config_home() -> Result<PathBuf> {
    // Respect XDG_CONFIG_HOME when it is set so custom config roots still work
    if let Ok(base) = env::var("XDG_CONFIG_HOME") {
        if !base.trim().is_empty() {
            return Ok(PathBuf::from(base));
        }
    }

    // Fall back to the conventional ~/.config path when XDG is unset
    Ok(home_dir()?.join(".config"))
}

pub(in crate::actions::hyprland) fn hyprland_config_target_in(
    config_home: &Path,
) -> HyprlandConfigTarget {
    let hypr_dir = config_home.join("hypr");
    let lua_path = hypr_dir.join("hyprland.lua");
    let conf_path = hypr_dir.join("hyprland.conf");

    // Hyprland 0.55 loads hyprland.lua before legacy hyprland.conf when both exist
    if lua_path.exists() || !conf_path.exists() {
        // A missing config still resolves to Lua so new installs do not create deprecated syntax
        return HyprlandConfigTarget {
            path: lua_path,
            syntax: HyprlandConfigSyntax::Lua,
        };
    }

    HyprlandConfigTarget {
        path: conf_path,
        syntax: HyprlandConfigSyntax::Hyprlang,
    }
}

pub(in crate::actions::hyprland) fn existing_hyprland_config_targets_in(
    config_home: &Path,
) -> Vec<HyprlandConfigTarget> {
    let hypr_dir = config_home.join("hypr");
    let candidates = [
        HyprlandConfigTarget {
            path: hypr_dir.join("hyprland.lua"),
            syntax: HyprlandConfigSyntax::Lua,
        },
        HyprlandConfigTarget {
            path: hypr_dir.join("hyprland.conf"),
            syntax: HyprlandConfigSyntax::Hyprlang,
        },
    ];

    // Cleanup must see both formats so stale managed blocks do not survive a syntax migration
    candidates
        .into_iter()
        .filter(|target| target.path.exists())
        .collect()
}
