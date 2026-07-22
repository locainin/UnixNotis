//! Shell startup file PATH entry helpers

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};
use unixnotis_core::filesystem::write_file_atomic_preserving_mode;

use crate::paths::format_with_home;
use crate::write_target::reject_unsafe_write_target;

use super::super::{log_line, ActionContext};

const PATH_BLOCK_MARKER: &str = "# unixnotis-installer path entry";

pub fn ensure_shell_path_entry(ctx: &mut ActionContext) -> Result<()> {
    // Startup file edits only affect new shells, not the current terminal
    let home = crate::paths::home_dir()?;
    let shell = env::var("SHELL").ok();
    let startup_files = shell_startup_files(&home, shell.as_deref());
    let mut updated_files = Vec::new();

    for startup_file in startup_files {
        if ensure_path_entry_in_file(&startup_file, &home, &ctx.paths.bin_dir)? {
            updated_files.push(startup_file);
        }
    }

    let rendered_bin = format_path_for_shell_line(&home, &ctx.paths.bin_dir);
    if updated_files.is_empty() {
        log_line(
            ctx,
            format!("Shell startup already includes PATH entry for {rendered_bin}"),
        );
    } else {
        for startup_file in updated_files {
            log_line(
                ctx,
                format!(
                    "Added PATH entry to {} so new terminals can run noticenterctl",
                    format_with_home(&startup_file)
                ),
            );
        }
    }

    Ok(())
}

pub fn remove_shell_path_entry(ctx: &mut ActionContext) -> Result<()> {
    // Resolve the user's home directory so shell startup files and path entries
    // can be located relative to the current user
    let home = crate::paths::home_dir()?;

    // Read the active shell when available so the cleanup can target the startup
    // files that are most likely to contain the installer-added PATH entry
    let shell = env::var("SHELL").ok();

    // Build the list of shell startup files that should be checked for PATH entries
    let startup_files = shell_startup_files(&home, shell.as_deref());

    // Track which files were actually modified so the final log output is accurate
    let mut removed_files = Vec::new();

    for startup_file in startup_files {
        // Remove only installer-owned references to the configured bin directory
        // Files that do not contain such an entry are left untouched
        if remove_path_entry_from_file(&startup_file, &home, &ctx.paths.bin_dir)? {
            removed_files.push(startup_file);
        }
    }

    if removed_files.is_empty() {
        // Nothing matched the installer-managed PATH entry, so no startup file changed
        log_line(ctx, "No installer-owned shell PATH entries found.");
    } else {
        for startup_file in removed_files {
            // Report each modified startup file using a home-relative display path
            // to keep the message readable and user-specific
            log_line(
                ctx,
                format!(
                    "Removed installer-owned PATH entry from {}",
                    format_with_home(&startup_file)
                ),
            );
        }
    }

    Ok(())
}

pub(in crate::actions::environment) fn shell_startup_files(
    home: &Path,
    shell: Option<&str>,
) -> Vec<PathBuf> {
    // Update the active shell rc first, then `.profile` as a fallback
    let mut files = Vec::new();
    let mut push_unique = |path: PathBuf| {
        if !files.contains(&path) {
            files.push(path);
        }
    };

    match shell.unwrap_or_default() {
        s if s.ends_with("zsh") => push_unique(home.join(".zshrc")),
        s if s.ends_with("bash") => push_unique(home.join(".bashrc")),
        _ => {}
    }

    push_unique(home.join(".profile"));
    files
}

pub(in crate::actions::environment) fn ensure_path_entry_in_file(
    file: &Path,
    home: &Path,
    bin_dir: &Path,
) -> Result<bool> {
    reject_unsafe_write_target(file)
        .map_err(|err| anyhow!("failed to write {}: {}", file.display(), err))?;
    let existing = match fs::read_to_string(file) {
        Ok(contents) => contents,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(err) => return Err(anyhow!("failed to read {}: {}", file.display(), err)),
    };

    if existing.contains(PATH_BLOCK_MARKER) || shell_path_entry_exists(&existing, home, bin_dir) {
        return Ok(false);
    }

    let export_line = format!(
        "export PATH=\"{}:$PATH\"",
        format_path_for_shell_line(home, bin_dir)
    );
    let mut updated = existing;
    if !updated.is_empty() && !updated.ends_with('\n') {
        updated.push('\n');
    }
    updated.push_str(PATH_BLOCK_MARKER);
    updated.push('\n');
    updated.push_str(&export_line);
    updated.push('\n');

    write_file_atomic_preserving_mode(file, updated.as_bytes(), 0o644)
        .map_err(|err| anyhow!("failed to write {}: {}", file.display(), err))?;
    Ok(true)
}

pub(in crate::actions::environment) fn remove_path_entry_from_file(
    file: &Path,
    home: &Path,
    bin_dir: &Path,
) -> Result<bool> {
    // Read the startup file contents before attempting any removal
    // Missing files are not an error because not every shell uses every startup file
    reject_unsafe_write_target(file)
        .map_err(|err| anyhow!("failed to write {}: {}", file.display(), err))?;
    let existing = match fs::read_to_string(file) {
        Ok(contents) => contents,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(err) => return Err(anyhow!("failed to read {}: {}", file.display(), err)),
    };

    // If the installer marker is not present, this file was not modified by this installer
    if !existing.contains(PATH_BLOCK_MARKER) {
        return Ok(false);
    }

    // Track whether anything was removed while collecting the lines that should remain
    let mut changed = false;
    let mut kept = Vec::new();

    // Use a peekable iterator so a marker line can inspect the following PATH line
    // before deciding whether both should be removed
    let mut lines = existing.lines().peekable();

    while let Some(line) = lines.next() {
        // Installer-owned PATH entries are stored as a marker line followed by
        // the actual shell export line
        if line.trim() == PATH_BLOCK_MARKER {
            if let Some(next) = lines.peek() {
                // Remove the marker and the following PATH entry only when the next
                // line points to the installer-managed bin directory
                if is_shell_path_entry_line(next.trim(), home, bin_dir) {
                    lines.next();
                    changed = true;
                    continue;
                }
            }
        }

        // Lines that are not part of an installer-owned PATH block are preserved
        kept.push(line);
    }

    // Avoid rewriting the file when no matching PATH block was removed
    if !changed {
        return Ok(false);
    }

    // Rebuild the file using newline separators and preserve a trailing newline
    // when the resulting file is not empty
    let mut updated = kept.join("\n");
    if !updated.is_empty() {
        updated.push('\n');
    }

    // Write the cleaned startup file back to disk
    write_file_atomic_preserving_mode(file, updated.as_bytes(), 0o644)
        .map_err(|err| anyhow!("failed to write {}: {}", file.display(), err))?;
    Ok(true)
}

pub(in crate::actions::environment) fn shell_path_entry_exists(
    contents: &str,
    home: &Path,
    bin_dir: &Path,
) -> bool {
    // Check each line independently after trimming surrounding whitespace so
    // indentation does not prevent detecting the PATH export
    contents
        .lines()
        .any(|line| is_shell_path_entry_line(line.trim(), home, bin_dir))
}

fn is_shell_path_entry_line(trimmed: &str, home: &Path, bin_dir: &Path) -> bool {
    // Build both the shell-friendly form and the absolute form because existing
    // startup files may contain either representation
    let shell_path = format_path_for_shell_line(home, bin_dir);
    let absolute_path = bin_dir.display().to_string();

    // Only consider export PATH lines, and only when they reference the installer
    // bin directory in either supported path format
    trimmed.starts_with("export PATH=")
        && (trimmed.contains(&shell_path) || trimmed.contains(&absolute_path))
}

pub(in crate::actions::environment) fn format_path_for_shell_line(
    home: &Path,
    bin_dir: &Path,
) -> String {
    // Prefer `$HOME` when possible so startup files stay portable across usernames
    if let Ok(stripped) = bin_dir.strip_prefix(home) {
        let tail = stripped.to_string_lossy();

        // If the bin directory is exactly the home directory, `$HOME` alone is enough
        if tail.is_empty() {
            "$HOME".to_string()
        } else {
            // Convert the home-relative suffix into a shell-friendly `$HOME/...` path
            format!("$HOME/{}", tail.trim_start_matches('/'))
        }
    } else {
        // Fall back to the absolute path when the bin directory is outside home
        bin_dir.display().to_string()
    }
}
