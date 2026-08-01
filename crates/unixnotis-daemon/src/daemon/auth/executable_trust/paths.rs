//! Trusted executable path matching

use std::os::unix::io::AsFd;
use std::path::{Path, PathBuf};

#[cfg(target_os = "linux")]
use super::super::policy::TRUSTED_CONTROL_EXECUTABLES;
#[cfg(not(target_os = "linux"))]
use super::super::policy::{TrustedExecutableSnapshot, TRUSTED_CONTROL_EXECUTABLES};
use super::fingerprint::{file_fingerprint, file_fingerprint_from_fd};
use super::metadata::trusted_control_file_metadata_is_safe;
use super::snapshots::trusted_control_snapshot;

pub(in crate::daemon) fn canonicalize_best_effort(path: &Path) -> PathBuf {
    // Missing paths remain raw so later trust comparisons fail as ordinary mismatches
    std::fs::canonicalize(path).unwrap_or_else(|_error| path.to_path_buf())
}

#[cfg(not(target_os = "linux"))]
pub(in crate::daemon) fn is_trusted_control_executable_path(path: &Path, relaxed: bool) -> bool {
    // Trust only known sibling binaries from the daemon install/build directory
    let Some(trusted_dir) = trusted_control_directory() else {
        return false;
    };

    let observed = canonicalize_best_effort(path);
    let Some(observed_name) = observed.file_name().and_then(|name| name.to_str()) else {
        return false;
    };
    if !TRUSTED_CONTROL_EXECUTABLES.contains(&observed_name) {
        return false;
    }

    if relaxed {
        return is_trusted_control_executable_path_relaxed_in_dir(&observed, &trusted_dir);
    }

    let Some(snapshot) = trusted_control_snapshot(&trusted_dir, observed_name) else {
        return false;
    };
    trusted_snapshot_matches_observed(&snapshot, &observed)
}

pub(in crate::daemon) fn is_trusted_control_executable_path_relaxed_in_dir(
    path: &Path,
    trusted_dir: &Path,
) -> bool {
    // Relaxed mode is only used for trial runs where local rebuilds are expected
    let Some(executable) = path.file_name().and_then(|name| name.to_str()) else {
        return false;
    };
    if !TRUSTED_CONTROL_EXECUTABLES.contains(&executable) {
        return false;
    }

    // Relaxed mode still rejects unsafe permission layouts
    let metadata = match std::fs::metadata(path) {
        Ok(metadata) => metadata,
        Err(_) => return false,
    };
    if !metadata.is_file() || !trusted_control_file_metadata_is_safe(&metadata) {
        return false;
    }

    // Keep trust scoped to known local build/install locations in trial mode
    trusted_path_matches_executable(trusted_dir, executable, path)
        || trusted_profile_sibling_matches_executable(trusted_dir, executable, path)
        || trusted_local_bin_matches_executable(executable, path)
}

pub(in crate::daemon) fn trusted_path_matches_executable(
    trusted_dir: &Path,
    executable: &str,
    observed: &Path,
) -> bool {
    let candidate = trusted_dir.join(executable);
    canonicalize_best_effort(&candidate) == observed
}

pub(in crate::daemon) fn trusted_profile_sibling_matches_executable(
    trusted_dir: &Path,
    executable: &str,
    observed: &Path,
) -> bool {
    // Developer trial runs often mix target/debug and target/release tools
    let profile = trusted_dir.file_name().and_then(|name| name.to_str());
    if !matches!(profile, Some("debug" | "release")) {
        return false;
    }
    let Some(target_root) = trusted_dir.parent() else {
        return false;
    };
    ["debug", "release"]
        .iter()
        .map(|variant| target_root.join(variant).join(executable))
        .any(|candidate| canonicalize_best_effort(&candidate) == observed)
}

pub(in crate::daemon) fn trusted_local_bin_matches_executable(
    executable: &str,
    observed: &Path,
) -> bool {
    // Installed keybinds usually point to ~/.local/bin during trial sessions
    let Some(home) = std::env::var_os("HOME") else {
        return false;
    };
    let candidate = PathBuf::from(home)
        .join(".local")
        .join("bin")
        .join(executable);
    canonicalize_best_effort(&candidate) == observed
}

fn trusted_control_directory() -> Option<PathBuf> {
    // The daemon trusts binaries installed next to the running daemon executable
    let current_exe = std::env::current_exe().ok()?;
    let current_exe = canonicalize_best_effort(&current_exe);
    current_exe.parent().map(Path::to_path_buf)
}

#[cfg(not(target_os = "linux"))]
pub(in crate::daemon::auth) fn trusted_snapshot_matches_observed(
    snapshot: &TrustedExecutableSnapshot,
    observed: &Path,
) -> bool {
    if snapshot.canonical_path != observed {
        return false;
    }

    // Live fingerprint must still match the pinned startup snapshot
    file_fingerprint(observed).is_some_and(|fingerprint| fingerprint == snapshot.fingerprint)
}

#[cfg(target_os = "linux")]
pub(in crate::daemon::auth) fn is_trusted_control_executable_from_fd<Fd: AsFd>(
    fd: &Fd,
    path: &Path,
    relaxed: bool,
) -> bool {
    // Trust only known sibling binaries from the daemon install/build directory
    let Some(trusted_dir) = trusted_control_directory() else {
        return false;
    };

    let observed = canonicalize_best_effort(path);
    let Some(observed_name) = observed.file_name().and_then(|name| name.to_str()) else {
        return false;
    };
    if !TRUSTED_CONTROL_EXECUTABLES.contains(&observed_name) {
        return false;
    }

    // Fingerprint the kernel file object via the descriptor, not the pathname.
    // This prevents the UNX-4-001 mount-namespace bypass where an attacker
    // shadows a trusted path with a different executable in their own namespace.
    let fingerprint = match file_fingerprint_from_fd(fd, path) {
        Some(fingerprint) => fingerprint,
        None => return false,
    };

    if relaxed {
        // Relaxed mode checks the path is in a trusted location, then verifies
        // the descriptor fingerprint matches the live file at that path
        if !is_trusted_control_executable_path_relaxed_in_dir(&observed, &trusted_dir) {
            return false;
        }
        // Verify the descriptor fingerprint matches what we'd get from the path
        file_fingerprint(path).is_some_and(|path_fingerprint| path_fingerprint == fingerprint)
    } else {
        // Strict mode: the descriptor fingerprint must match the startup snapshot
        let Some(snapshot) = trusted_control_snapshot(&trusted_dir, observed_name) else {
            return false;
        };
        fingerprint == snapshot.fingerprint
    }
}
