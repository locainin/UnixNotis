//! Path lookup and spawn setup for supervised UI children

use std::env;
use std::path::PathBuf;

fn resolve_sibling_binary(name: &str) -> Option<PathBuf> {
    let exe = env::current_exe().ok()?;
    let dir = exe.parent()?;

    // Prefer sibling binaries next to the daemon binary
    // This keeps local installs working without a PATH lookup
    let candidate = dir.join(name);
    if candidate.is_file() {
        return Some(candidate);
    }

    // Windows-style suffix support keeps mixed developer setups simple
    let candidate = dir.join(format!("{name}.exe"));
    if candidate.is_file() {
        return Some(candidate);
    }
    None
}

pub(super) fn resolve_popups_path() -> Option<PathBuf> {
    resolve_sibling_binary("unixnotis-popups")
}

pub(super) fn resolve_center_path() -> Option<PathBuf> {
    resolve_sibling_binary("unixnotis-center")
}

#[cfg(test)]
#[path = "tests/paths.rs"]
mod tests;
