//! Shell startup file PATH entry helpers

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};

use crate::paths::format_with_home;

use super::super::{log_line, ActionContext};

const PATH_BLOCK_MARKER: &str = "# unixnotis-installer path entry";

pub(crate) fn ensure_shell_path_entry(ctx: &mut ActionContext) -> Result<()> {
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
    let existing = match fs::read_to_string(file) {
        Ok(contents) => contents,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(err) => return Err(anyhow!("failed to read {}: {}", file.display(), err)),
    };

    if existing.contains(PATH_BLOCK_MARKER) || shell_path_entry_exists(&existing, home, bin_dir) {
        return Ok(false);
    }

    if let Some(parent) = file.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| anyhow!("failed to create {}: {}", parent.display(), err))?;
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

    fs::write(file, updated)
        .map_err(|err| anyhow!("failed to write {}: {}", file.display(), err))?;
    Ok(true)
}

pub(in crate::actions::environment) fn shell_path_entry_exists(
    contents: &str,
    home: &Path,
    bin_dir: &Path,
) -> bool {
    let shell_path = format_path_for_shell_line(home, bin_dir);
    let absolute_path = bin_dir.display().to_string();

    contents.lines().any(|line| {
        let trimmed = line.trim();
        trimmed.starts_with("export PATH=")
            && (trimmed.contains(&shell_path) || trimmed.contains(&absolute_path))
    })
}

pub(in crate::actions::environment) fn format_path_for_shell_line(
    home: &Path,
    bin_dir: &Path,
) -> String {
    // Prefer `$HOME` when possible so startup files stay portable across usernames
    if let Ok(stripped) = bin_dir.strip_prefix(home) {
        let tail = stripped.to_string_lossy();
        if tail.is_empty() {
            "$HOME".to_string()
        } else {
            format!("$HOME/{}", tail.trim_start_matches('/'))
        }
    } else {
        bin_dir.display().to_string()
    }
}
