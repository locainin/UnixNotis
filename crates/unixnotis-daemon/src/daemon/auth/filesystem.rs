//! Small filesystem helpers shared by authorization modules

use std::path::{Path, PathBuf};

pub(in crate::daemon) fn canonicalize_best_effort(path: &Path) -> PathBuf {
    // Fall back to the raw path so missing paths fail later as normal trust mismatches
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}
