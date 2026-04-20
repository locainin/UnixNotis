//! Hyprland bootstrap flow for install and uninstall

use std::fs;

use super::super::{log_line, ActionContext};
use super::block::strip_hyprland_bootstrap_block;
use super::detect::{
    has_exec_once_dbus_update, has_exec_once_import, has_exec_once_restart,
    hyprland_dbus_update_line, hyprland_import_line, HYPR_RESTART_CMD,
};
use super::paths::hyprland_config_path;
use crate::paths::format_with_home;

pub(in crate::actions) fn ensure_hyprland_autostart(ctx: &mut ActionContext) {
    // Resolve the canonical config path so includes do not get guessed here
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

    // Strip any managed block first so missing lines can be rebuilt cleanly
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

    // Add only the lines that are still missing from the live config
    let mut additions = Vec::new();
    if !has_exec_once_dbus_update(&stripped) {
        additions.push(hyprland_dbus_update_line());
    }
    if !has_exec_once_import(&stripped, &super::super::actions_env::HYPR_IMPORT_VARS) {
        additions.push(hyprland_import_line());
    }
    if !has_exec_once_restart(&stripped) {
        additions.push(HYPR_RESTART_CMD.to_string());
    }

    if additions.is_empty() {
        // If the live file already has everything, drop stale managed blocks and stop
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
    updated_contents.push_str(&super::block::render_hyprland_bootstrap_block(&additions));

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

pub(in crate::actions) fn remove_hyprland_autostart(ctx: &mut ActionContext) {
    // Remove only the managed block so user edits outside it stay intact
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
        // Incomplete markers are left alone to avoid destructive cleanup
        return;
    }
    if !strip_result.block_found {
        return;
    }

    if let Err(err) = fs::write(&hypr_config, strip_result.stripped) {
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
