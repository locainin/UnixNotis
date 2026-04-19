//! Hyprland config bootstrap management.
//!
//! Encapsulates the managed exec-once block for consistent install/uninstall behavior.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Result;

use crate::paths::{format_with_home, home_dir};

use super::{actions_env::HYPR_IMPORT_VARS, log_line, ActionContext};

const HYPR_BOOTSTRAP_START: &str = "# BEGIN UNIXNOTIS SESSION BOOTSTRAP";
const HYPR_BOOTSTRAP_END: &str = "# END UNIXNOTIS SESSION BOOTSTRAP";
const HYPR_RESTART_CMD: &str =
    "exec-once = systemctl --user --no-block restart unixnotis-daemon.service";

pub(super) fn ensure_hyprland_autostart(ctx: &mut ActionContext) {
    // Locate the canonical Hyprland config to avoid assumptions about custom includes.
    let hypr_config = match hyprland_config_path() {
        Ok(path) => path,
        Err(err) => {
            log_line(ctx, format!("Warning: {}", err));
            return;
        }
    };
    if !hypr_config.exists() {
        log_line(
            ctx,
            format!(
                "Hyprland config not found at {}; skipping env bootstrap",
                format_with_home(&hypr_config)
            ),
        );
        return;
    }

    let contents = match fs::read_to_string(&hypr_config) {
        Ok(contents) => contents,
        Err(err) => {
            log_line(
                ctx,
                format!(
                    "Warning: failed to read {}: {}",
                    format_with_home(&hypr_config),
                    err
                ),
            );
            return;
        }
    };

    // Remove any previously managed block so it can be rewritten cleanly.
    // If the block is malformed, the strip result keeps the file intact for safe appends.
    let strip_result = strip_hyprland_bootstrap_block(ctx, &contents, &hypr_config);
    if strip_result.malformed {
        log_line(
            ctx,
            format!(
                "Warning: malformed UnixNotis bootstrap block in {}; fix manually before reapplying",
                format_with_home(&hypr_config)
            ),
        );
        return;
    }
    let stripped = strip_result.stripped;
    let block_found = strip_result.block_found;

    // Only append missing exec-once directives; existing lines remain untouched.
    // Build the minimal set of exec-once directives required for a clean login sync.
    let mut additions = Vec::new();
    if !has_exec_once_dbus_update(&stripped) {
        additions.push(hyprland_dbus_update_line());
    }
    // Detect existing exec-once imports that already cover the required variables.
    let has_import = has_exec_once_import(&stripped, &HYPR_IMPORT_VARS);
    if !has_import {
        additions.push(hyprland_import_line());
    }
    // Detect exec-once restarts without matching commented examples.
    let has_restart = has_exec_once_restart(&stripped);
    if !has_restart {
        additions.push(HYPR_RESTART_CMD.to_string());
    }

    if additions.is_empty() {
        // When no directives are missing, drop any stale managed block and keep the file stable.
        if block_found {
            if let Err(err) = fs::write(&hypr_config, stripped) {
                log_line(
                    ctx,
                    format!("Warning: failed to update Hyprland config: {}", err),
                );
            } else {
                log_line(
                    ctx,
                    format!(
                        "Removed redundant UnixNotis bootstrap from {}",
                        format_with_home(&hypr_config)
                    ),
                );
            }
        }
        log_line(ctx, "Hyprland config already includes UnixNotis env sync");
        return;
    }

    let mut updated_contents = stripped;
    if !updated_contents.ends_with('\n') {
        updated_contents.push('\n');
    }
    // Append the managed block to preserve the existing user config ordering.
    updated_contents.push_str(&render_hyprland_bootstrap_block(&additions));

    if let Err(err) = fs::write(&hypr_config, updated_contents) {
        log_line(
            ctx,
            format!("Warning: failed to update Hyprland config: {}", err),
        );
    } else {
        log_line(
            ctx,
            format!(
                "Updated Hyprland config at {}",
                format_with_home(&hypr_config)
            ),
        );
    }
}

pub(super) fn remove_hyprland_autostart(ctx: &mut ActionContext) {
    // Only remove the managed block, leaving unrelated Hyprland config intact.
    let hypr_config = match hyprland_config_path() {
        Ok(path) => path,
        Err(err) => {
            log_line(ctx, format!("Warning: {}", err));
            return;
        }
    };
    if !hypr_config.exists() {
        return;
    }

    let contents = match fs::read_to_string(&hypr_config) {
        Ok(contents) => contents,
        Err(err) => {
            log_line(
                ctx,
                format!(
                    "Warning: failed to read {}: {}",
                    format_with_home(&hypr_config),
                    err
                ),
            );
            return;
        }
    };

    let strip_result = strip_hyprland_bootstrap_block(ctx, &contents, &hypr_config);
    if strip_result.malformed {
        // Avoid destructive edits when the managed block is incomplete.
        return;
    }
    let stripped = strip_result.stripped;
    let block_found = strip_result.block_found;
    if !block_found {
        return;
    }
    if let Err(err) = fs::write(&hypr_config, stripped) {
        log_line(
            ctx,
            format!("Warning: failed to update Hyprland config: {}", err),
        );
    } else {
        log_line(
            ctx,
            format!(
                "Removed UnixNotis bootstrap from {}",
                format_with_home(&hypr_config)
            ),
        );
    }
}

fn hyprland_config_path() -> Result<PathBuf> {
    // Respect XDG_CONFIG_HOME when defined to support non-default config roots.
    if let Ok(base) = env::var("XDG_CONFIG_HOME") {
        if !base.trim().is_empty() {
            return Ok(PathBuf::from(base).join("hypr").join("hyprland.conf"));
        }
    }
    // Fall back to the conventional ~/.config path when XDG_CONFIG_HOME is unset.
    Ok(home_dir()?
        .join(".config")
        .join("hypr")
        .join("hyprland.conf"))
}

fn render_hyprland_bootstrap_block(lines: &[String]) -> String {
    // The block markers allow a clean uninstall without touching unrelated config content.
    let mut block = String::new();
    block.push_str(HYPR_BOOTSTRAP_START);
    block.push('\n');
    block.push_str("# UnixNotis session bootstrap\n");
    block.push_str("# Ensures systemd user environment carries Wayland session variables.\n");
    block.push_str("# Managed by unixnotis-installer; safe to remove with uninstall.\n");
    for line in lines {
        block.push_str(line);
        block.push('\n');
    }
    block.push_str(HYPR_BOOTSTRAP_END);
    block.push('\n');
    block
}

fn hyprland_import_line() -> String {
    // Keep the import list in one place so Hyprland exec-once stays consistent.
    format!(
        "exec-once = systemctl --user import-environment {}",
        HYPR_IMPORT_VARS.join(" ")
    )
}

fn hyprland_dbus_update_line() -> String {
    // Keep login-time D-Bus env sync aligned with the installer runtime path.
    format!("exec-once = {}", hyprland_dbus_update_command())
}

fn has_exec_once_command(contents: &str, needle: &str) -> bool {
    // Only consider non-comment exec-once lines to avoid false positives.
    contents.lines().any(|line| {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            return false;
        }
        trimmed.starts_with("exec-once") && trimmed.contains(needle)
    })
}

fn has_exec_once_dbus_update(contents: &str) -> bool {
    // Accept both the new explicit import list and the older `--systemd --all`
    // form so existing manual setups do not grow duplicate exec-once lines.
    // That keeps upgrades quiet without forcing an immediate config rewrite
    has_exec_once_command(
        contents,
        "dbus-update-activation-environment --systemd --all",
    ) || has_exec_once_command(contents, &hyprland_dbus_update_command())
}

fn hyprland_dbus_update_command() -> String {
    format!(
        "dbus-update-activation-environment {}",
        HYPR_IMPORT_VARS.join(" ")
    )
}

fn has_exec_once_import(contents: &str, vars: &[&str]) -> bool {
    // Ensure the import line includes every expected variable.
    contents.lines().any(|line| {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            return false;
        }
        if !(trimmed.starts_with("exec-once") && trimmed.contains("import-environment")) {
            return false;
        }
        vars.iter().all(|var| trimmed.contains(var))
    })
}

fn has_exec_once_restart(contents: &str) -> bool {
    // Require an active exec-once restart line for unixnotis-daemon.service.
    contents.lines().any(|line| {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            return false;
        }
        trimmed.starts_with("exec-once")
            && trimmed.contains("systemctl")
            && trimmed.contains("restart")
            && trimmed.contains("unixnotis-daemon.service")
    })
}

struct HyprlandStripResult {
    stripped: String,
    block_found: bool,
    malformed: bool,
}

fn strip_hyprland_bootstrap_block(
    ctx: &mut ActionContext,
    contents: &str,
    config_path: &Path,
) -> HyprlandStripResult {
    // Use the marker range to avoid disturbing user-maintained content.
    let original = contents.to_string();
    let mut current = contents.to_string();
    let mut removed_any = false;

    loop {
        let Some(start) = current.find(HYPR_BOOTSTRAP_START) else {
            return HyprlandStripResult {
                stripped: current,
                block_found: removed_any,
                malformed: false,
            };
        };
        let Some(end_rel) = current[start..].find(HYPR_BOOTSTRAP_END) else {
            log_line(
                ctx,
                format!(
                    "Warning: unterminated UnixNotis block in {}; leaving content intact and appending a fresh block",
                    format_with_home(config_path)
                ),
            );
            return HyprlandStripResult {
                stripped: original,
                block_found: false,
                malformed: true,
            };
        };
        let end = start + end_rel + HYPR_BOOTSTRAP_END.len();
        let before = current[..start].trim_end_matches('\n');
        let after = current[end..].trim_start_matches('\n');
        let mut merged = String::new();
        merged.push_str(before);
        if !before.is_empty() && !after.is_empty() {
            merged.push('\n');
        }
        merged.push_str(after);
        current = merged;
        removed_any = true;
    }
}

#[cfg(test)]
#[path = "actions_hyprland_tests.rs"]
mod tests;
