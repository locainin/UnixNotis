//! Hyprland bootstrap flow for install and uninstall

use std::fs;

use super::super::{log_line, ActionContext};
use super::block::strip_hyprland_bootstrap_block;
use super::detect::{
    has_import_command_with_vars, has_legacy_dbus_update, has_startup_command,
    hyprland_startup_line,
};
use super::paths::{existing_hyprland_config_targets, hyprland_config_target};
use crate::paths::format_with_home;

pub(in crate::actions) fn ensure_hyprland_autostart(ctx: &mut ActionContext) {
    // Resolve the active top-level config before deciding which syntax to write
    let target = match hyprland_config_target() {
        Ok(target) => target,
        Err(err) => {
            log_line(ctx, format!("Warning: {}", err));
            return;
        }
    };
    let hypr_config = target.path;
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
    // User-managed equivalents outside the installer block should not be duplicated
    for command in ctx
        .paths
        .service
        .hyprland_startup_commands(&super::super::HYPR_IMPORT_VARS)
    {
        if hyprland_command_present(&stripped, &command) {
            continue;
        }
        additions.push(hyprland_startup_line(target.syntax, &command));
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
    updated_contents.push_str(&super::block::render_hyprland_bootstrap_block(
        target.syntax,
        &additions,
    ));

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

fn hyprland_command_present(contents: &str, command: &str) -> bool {
    if command.starts_with("dbus-update-activation-environment") {
        return has_legacy_dbus_update(contents) || has_startup_command(contents, command);
    }
    if command.contains("import-environment") {
        return has_import_command_with_vars(contents, &super::super::HYPR_IMPORT_VARS);
    }
    has_startup_command(contents, command)
}

pub(in crate::actions) fn remove_hyprland_autostart(ctx: &mut ActionContext) {
    // Remove only the managed block so user edits outside it stay intact
    let targets = match existing_hyprland_config_targets() {
        Ok(targets) => targets,
        Err(err) => {
            log_line(ctx, format!("Warning: {}", err));
            return;
        }
    };

    for target in targets {
        // Cleanup is best-effort per file so one broken config does not block the other format
        let hypr_config = target.path;
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
                continue;
            }
        };

        let strip_result = strip_hyprland_bootstrap_block(ctx, &contents, &hypr_config);
        if strip_result.malformed {
            // Incomplete markers are left alone to avoid destructive cleanup
            continue;
        }
        if !strip_result.block_found {
            continue;
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
}
