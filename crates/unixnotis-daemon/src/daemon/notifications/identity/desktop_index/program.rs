//! Desktop launch-program parsing and path resolution

use std::path::{Path, PathBuf};

pub(super) fn resolve_program(program: &Path) -> Option<PathBuf> {
    // Canonical paths are presentation data while device and inode carry the proof
    if program.is_absolute() {
        return program.canonicalize().ok();
    }
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|directory| directory.join(program))
        .find_map(|candidate| candidate.canonicalize().ok())
}
