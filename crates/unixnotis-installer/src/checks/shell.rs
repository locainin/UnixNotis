//! Shell PATH checks for installed binaries

use std::env;
use std::path::Path;

use crate::paths::{format_with_home, InstallPaths};

use super::CheckItem;

pub(super) fn path_check_item(paths: &InstallPaths) -> CheckItem {
    let rendered_bin = format_with_home(&paths.bin_dir);
    // PATH alone is not enough here because uninstall can remove the command
    // while the shell still keeps the bin dir in its search path
    let path_ready = path_includes_bin(paths);
    // Check the managed install location so the status matches real command availability
    let command_installed = install_bin_has_command(paths, "noticenterctl");
    path_check_item_from(&rendered_bin, path_ready, command_installed)
}

fn path_includes_bin(paths: &InstallPaths) -> bool {
    let Ok(path_var) = env::var("PATH") else {
        return false;
    };
    // Canonical path matching avoids duplicate warn states from symlinked bin paths
    env::split_paths(&path_var).any(|entry| path_entries_match(&entry, &paths.bin_dir))
}

fn install_bin_has_command(paths: &InstallPaths, command: &str) -> bool {
    // Check the managed install directory directly instead of relying on PATH search
    paths.bin_dir.join(command).is_file()
}

fn path_check_item_from(
    rendered_bin: &str,
    path_ready: bool,
    command_installed: bool,
) -> CheckItem {
    match (path_ready, command_installed) {
        // This is the only fully ready state for direct command use
        (true, true) => CheckItem::ok(
            "Shell PATH",
            &format!("includes {rendered_bin}; noticenterctl is installed there"),
        ),
        // Uninstall can leave PATH intact, so this still needs to warn
        (true, false) => CheckItem::warn(
            "Shell PATH",
            &format!(
                "includes {rendered_bin}, but noticenterctl is not installed there right now"
            ),
        ),
        // Install can finish before the current shell reloads its startup files
        (false, true) => CheckItem::warn(
            "Shell PATH",
            &format!(
                "missing {rendered_bin}; noticenterctl is installed there, but the current terminal session cannot run it directly until the shell reloads PATH or a new terminal is opened"
            ),
        ),
        // Fresh systems hit this path before the first install
        (false, false) => CheckItem::warn(
            "Shell PATH",
            &format!(
                "missing {rendered_bin}; noticenterctl is not installed there right now"
            ),
        ),
    }
}

fn path_entries_match(entry: &Path, target: &Path) -> bool {
    if entry == target {
        return true;
    }

    match (entry.canonicalize(), target.canonicalize()) {
        (Ok(left), Ok(right)) => left == right,
        _ => false,
    }
}

#[cfg(test)]
#[path = "tests/shell.rs"]
mod tests;
