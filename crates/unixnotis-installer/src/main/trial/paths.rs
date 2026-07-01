//! PATH lookup, filesystem probes, and shell quoting for trial mode

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

pub(super) fn path_entries() -> Vec<PathBuf> {
    // Empty PATH is treated as no available shell command locations
    let Ok(path_var) = env::var("PATH") else {
        return Vec::new();
    };
    env::split_paths(&path_var).collect()
}

pub(super) fn find_command_on_path_with_index(
    command: &str,
    entries: &[PathBuf],
) -> Option<(usize, PathBuf)> {
    // Return the first command because that is what shell lookup will execute
    entries.iter().enumerate().find_map(|(index, entry)| {
        let candidate = entry.join(command);
        // PATH shadowing should follow executable commands, not plain files with matching names
        if is_executable_file(&candidate) {
            Some((index, candidate))
        } else {
            None
        }
    })
}

#[cfg(unix)]
fn is_executable_file(path: &Path) -> bool {
    let Ok(metadata) = fs::metadata(path) else {
        return false;
    };
    // Shell lookup needs an executable regular file, not just a matching filename
    // Any execute bit counts because the current user may be owner, group, or other
    metadata.file_type().is_file() && metadata.permissions().mode() & 0o111 != 0
}

#[cfg(not(unix))]
fn is_executable_file(path: &Path) -> bool {
    // Non-Unix test builds keep the old conservative regular-file behavior
    path.is_file()
}

pub(super) fn path_entries_match(left: &Path, right: &Path) -> bool {
    // Fast path avoids filesystem work for normal exact entries
    if left == right {
        return true;
    }
    // Canonical comparison lets symlinked PATH entries match the real directory
    match (left.canonicalize(), right.canonicalize()) {
        (Ok(lhs), Ok(rhs)) => lhs == rhs,
        _ => false,
    }
}

pub(super) fn path_dir_is_writable(dir: &Path) -> bool {
    // create_new avoids touching any existing file in the target directory
    let probe = dir.join(format!(
        ".unixnotis-trial-write-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0)
    ));
    match std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&probe)
    {
        Ok(_) => {
            // Probe file is trial-only and should not outlive the writability check
            let _ = fs::remove_file(probe);
            true
        }
        Err(_) => false,
    }
}

pub(super) fn path_exists_no_follow(path: &Path) -> bool {
    // symlink_metadata catches dangling symlinks that exists() would miss
    fs::symlink_metadata(path).is_ok()
}

pub(super) fn canonicalize_best_effort(path: &Path) -> PathBuf {
    // Missing paths still need stable comparison behavior
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

pub(super) fn shell_quote(value: &str) -> String {
    // Single-quote shell escaping keeps paths with spaces or quotes intact
    let mut quoted = String::from("'");
    for ch in value.chars() {
        if ch == '\'' {
            quoted.push_str("'\"'\"'");
        } else {
            quoted.push(ch);
        }
    }
    quoted.push('\'');
    quoted
}
