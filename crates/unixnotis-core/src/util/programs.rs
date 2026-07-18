//! Executable discovery with trusted-path support

use std::collections::HashMap;
use std::env;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

struct ProgramCache {
    // Snapshot of PATH used to invalidate cached entries when environment changes
    path: Option<String>,
    // Cached program presence results keyed by program name
    results: HashMap<String, bool>,
}

static PROGRAM_CACHE: OnceLock<Mutex<ProgramCache>> = OnceLock::new();
pub const TRUSTED_SYSTEM_TOOL_DIRS: [&str; 4] = ["/usr/bin", "/bin", "/usr/sbin", "/sbin"];

/// Check whether a program exists in $PATH, caching results to avoid repeated scans
pub fn program_in_path(program: &str) -> bool {
    if program.contains(std::path::MAIN_SEPARATOR) {
        return is_executable_path(Path::new(program));
    }
    // Capture PATH once per call to avoid repeated environment lookups
    let current_path = env::var("PATH").ok();
    let cache = PROGRAM_CACHE.get_or_init(|| {
        Mutex::new(ProgramCache {
            path: None,
            results: HashMap::new(),
        })
    });
    if let Ok(mut cache) = cache.lock() {
        // Reset cached lookups whenever PATH changes to avoid stale long-lived results
        if cache.path.as_deref() != current_path.as_deref() {
            cache.path = current_path.clone();
            cache.results.clear();
        }
        if let Some(result) = cache.results.get(program) {
            return *result;
        }

        let found = current_path.as_ref().is_some_and(|paths| {
            env::split_paths(paths).any(|dir| is_executable_path(&dir.join(program)))
        });
        cache.results.insert(program.to_string(), found);
        return found;
    }

    // A poisoned cache must not disable program discovery for the rest of the process
    current_path.as_ref().is_some_and(|paths| {
        env::split_paths(paths).any(|dir| is_executable_path(&dir.join(program)))
    })
}

#[must_use]
pub fn trusted_system_program_path(program: &str) -> Option<PathBuf> {
    if program.is_empty() || program.contains(std::path::MAIN_SEPARATOR) {
        return None;
    }

    // Security-sensitive helpers use an explicit FHS policy instead of attacker-controlled PATH
    TRUSTED_SYSTEM_TOOL_DIRS
        .iter()
        .map(|directory| Path::new(directory).join(program))
        .find(|path| is_executable_path(path))
}

fn is_executable_path(path: &Path) -> bool {
    // Backend selection only succeeds when the target is a regular executable
    let Ok(metadata) = std::fs::metadata(path) else {
        return false;
    };
    if !metadata.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        metadata.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        true
    }
}

#[cfg(test)]
#[path = "tests/programs.rs"]
mod tests;
