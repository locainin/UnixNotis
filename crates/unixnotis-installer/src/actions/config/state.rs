use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use unixnotis_core::util;

use crate::paths::format_with_home;

use super::super::{log_line, ActionContext};

pub(in crate::actions::config) const DND_STATE_FILE: &str = "state.json";

pub(crate) fn remove_state(ctx: &mut ActionContext) -> Result<()> {
    let Some(state_dir) = resolve_state_dir() else {
        log_line(ctx, "State directory not resolved; skipping state cleanup.");
        return Ok(());
    };

    let state_root = state_dir.join("unixnotis");
    let outcome = match remove_state_file(&state_root) {
        Ok(outcome) => outcome,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            log_line(ctx, "State file not present; nothing to remove.");
            return Ok(());
        }
        Err(err) => return Err(err).with_context(|| "failed to remove state file"),
    };

    if outcome.removed_file {
        let path = state_root.join(DND_STATE_FILE);
        log_line(
            ctx,
            format!(
                "Removed persisted state file: {}",
                format_with_state_env(&path)
            ),
        );
    }

    if outcome.removed_dir {
        log_line(
            ctx,
            format!(
                "Removed empty state directory: {}",
                format_with_state_env(&state_root)
            ),
        );
    }
    if let Some(warning) = outcome.cleanup_warning {
        log_line(ctx, warning);
    }

    Ok(())
}

#[derive(Debug, Default)]
pub(in crate::actions::config) struct RemoveStateOutcome {
    // True when state.json was deleted
    pub(in crate::actions::config) removed_file: bool,
    // True when the now-empty unixnotis state dir was deleted too
    pub(in crate::actions::config) removed_dir: bool,
    // Optional warning for cleanup work that failed after file removal
    pub(in crate::actions::config) cleanup_warning: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::actions::config) enum DirCleanupOutcome {
    // State dir was empty and removal worked
    Removed,
    // State dir still had other files, so it was left in place
    KeptNotEmpty,
    // read_dir failed, so emptiness could not be checked
    InspectFailed,
    // Dir looked empty but remove_dir failed
    RemoveFailed,
}

pub(in crate::actions::config) fn remove_state_file(
    state_root: &Path,
) -> std::io::Result<RemoveStateOutcome> {
    let state_file = state_root.join(DND_STATE_FILE);
    // Remove the persisted DND file first because that is the main cleanup target
    let removed_file = match fs::remove_file(&state_file) {
        Ok(()) => true,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => false,
        Err(err) => return Err(err),
    };

    if !removed_file {
        // Nothing changed, so there is no follow-up directory cleanup to attempt
        return Ok(RemoveStateOutcome::default());
    }

    // Cleanup is best-effort, but the exact result is tracked so failures are not hidden
    let cleanup_outcome = cleanup_empty_state_dir(state_root);
    let removed_dir = matches!(cleanup_outcome, DirCleanupOutcome::Removed);
    // Build a user-facing warning only for real cleanup failures
    let cleanup_warning = cleanup_warning_message(state_root, cleanup_outcome);

    Ok(RemoveStateOutcome {
        removed_file,
        removed_dir,
        cleanup_warning,
    })
}

fn is_dir_empty(path: &Path) -> std::io::Result<bool> {
    let mut entries = fs::read_dir(path)?;
    Ok(entries.next().is_none())
}

fn cleanup_empty_state_dir(state_root: &Path) -> DirCleanupOutcome {
    // A non-empty dir is normal because other state files may exist later
    match is_dir_empty(state_root) {
        Ok(false) => DirCleanupOutcome::KeptNotEmpty,
        // Only try removing the dir after confirming it is empty
        Ok(true) => match fs::remove_dir(state_root) {
            Ok(()) => DirCleanupOutcome::Removed,
            Err(_) => DirCleanupOutcome::RemoveFailed,
        },
        // Surface read_dir problems separately so they can be logged upstream
        Err(_) => DirCleanupOutcome::InspectFailed,
    }
}

pub(in crate::actions::config) fn cleanup_warning_message(
    state_root: &Path,
    outcome: DirCleanupOutcome,
) -> Option<String> {
    match outcome {
        // Normal paths stay quiet to avoid noisy uninstall output
        DirCleanupOutcome::Removed | DirCleanupOutcome::KeptNotEmpty => None,
        // The file is already gone here, so the warning is about leftover directory state
        DirCleanupOutcome::InspectFailed => Some(format!(
            "Warning: failed to inspect state directory after removing {}: {}",
            DND_STATE_FILE,
            format_with_state_env(state_root)
        )),
        // This warns only when the dir should have been removable but was not
        DirCleanupOutcome::RemoveFailed => Some(format!(
            "Warning: failed to remove empty state directory after removing {}: {}",
            DND_STATE_FILE,
            format_with_state_env(state_root)
        )),
    }
}

fn resolve_state_dir() -> Option<PathBuf> {
    util::resolve_state_dir()
}

pub(in crate::actions::config) fn format_with_state_env(path: &Path) -> String {
    // Prefer XDG_STATE_HOME for display when available to avoid absolute paths in logs.
    if let Ok(state_home) = std::env::var("XDG_STATE_HOME") {
        if !state_home.trim().is_empty() {
            let state_root = PathBuf::from(state_home);
            if let Ok(stripped) = path.strip_prefix(&state_root) {
                let mut rendered = PathBuf::from("$XDG_STATE_HOME");
                rendered.push(stripped);
                return rendered.display().to_string();
            }
        }
    }

    format_with_home(path)
}
