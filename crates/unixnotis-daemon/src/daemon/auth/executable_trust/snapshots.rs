//! Startup-time trusted executable snapshots

use std::collections::HashMap;
use std::path::Path;

use super::super::policy::{TrustedExecutableSnapshot, TRUSTED_CONTROL_EXECUTABLES};
use super::fingerprint::file_fingerprint;
use super::paths::canonicalize_best_effort;

pub(in crate::daemon) fn build_trusted_control_snapshots(
    trusted_dir: &Path,
) -> HashMap<String, TrustedExecutableSnapshot> {
    let mut snapshots = HashMap::new();
    for executable in TRUSTED_CONTROL_EXECUTABLES {
        let Some(snapshot) = build_trusted_control_snapshot(trusted_dir, executable) else {
            continue;
        };
        snapshots.insert(executable.to_string(), snapshot);
    }
    snapshots
}

fn build_trusted_control_snapshot(
    trusted_dir: &Path,
    executable: &str,
) -> Option<TrustedExecutableSnapshot> {
    // Missing sibling means this executable is not trusted in strict mode
    let candidate = trusted_dir.join(executable);
    if !candidate.is_file() {
        return None;
    }

    let canonical = canonicalize_best_effort(&candidate);
    let fingerprint = file_fingerprint(&canonical)?;
    Some(TrustedExecutableSnapshot {
        canonical_path: canonical,
        fingerprint,
    })
}
