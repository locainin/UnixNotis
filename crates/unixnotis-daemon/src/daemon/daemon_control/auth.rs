//! Authorization helpers for privileged control methods
//!
//! This file isolates caller validation so the interface file can focus on D-Bus methods

use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::OnceLock;

use rustix::process::geteuid;
use sha2::{Digest, Sha256};
use tracing::warn;
use zbus::fdo::DBusProxy;
use zbus::message::Header;

use crate::daemon::{to_fdo_error, DaemonState};

// Only these binaries are allowed to call privileged control methods
const TRUSTED_CONTROL_EXECUTABLES: [&str; 4] = [
    "noticenterctl",
    "unixnotis-center",
    "unixnotis-popups",
    "unixnotis-daemon",
];

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct TrustedExecutableSnapshot {
    // Canonical path ties the trust decision to one concrete on-disk binary
    canonical_path: PathBuf,
    // Fingerprint blocks same-path replacement after the daemon has started
    fingerprint: FileFingerprint,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct FileFingerprint {
    // File length catches most replacement attempts before hashing even matters
    len: u64,
    // Content hash blocks exact-name swaps inside a writable sibling directory
    sha256: [u8; 32],
}

pub(super) async fn authorize_control_call(
    state: &Arc<DaemonState>,
    header: &Header<'_>,
    method: &'static str,
) -> zbus::fdo::Result<()> {
    // One guard for all control calls
    let sender = header
        .sender()
        .ok_or_else(|| zbus::fdo::Error::AccessDenied("missing sender".to_string()))?;
    let sender_name = sender.as_str().to_string();

    // Ask the bus for sender metadata so payload fields cannot spoof identity
    let proxy = DBusProxy::new(state.connection())
        .await
        .map_err(to_fdo_error)?;
    let bus_name = zbus::names::BusName::try_from(sender_name.as_str())
        .map_err(|_| zbus::fdo::Error::AccessDenied("invalid sender".to_string()))?;

    // Only the current desktop user can control panel behavior
    let caller_uid = proxy.get_connection_unix_user(bus_name.clone()).await?;
    let expected_uid = geteuid().as_raw();
    if caller_uid != expected_uid {
        warn!(
            method,
            sender = %sender_name,
            uid = caller_uid,
            expected_uid,
            "rejected control caller with mismatched uid"
        );
        return Err(zbus::fdo::Error::AccessDenied(
            "caller uid is not authorized for control operation".to_string(),
        ));
    }

    // Same user is not enough
    // The caller path must match a trusted sibling binary
    let pid = proxy.get_connection_unix_process_id(bus_name).await?;
    let exe_path = read_process_executable_path(pid).await;
    if !exe_path
        .as_deref()
        .is_some_and(is_trusted_control_executable_path)
    {
        warn!(
            method,
            sender = %sender_name,
            pid,
            executable = exe_path
                .as_ref()
                .map(|path| path.display().to_string())
                .unwrap_or_else(|| "unknown".to_string()),
            "rejected untrusted control caller"
        );
        return Err(zbus::fdo::Error::AccessDenied(
            "caller is not authorized for control operation".to_string(),
        ));
    }

    Ok(())
}

#[cfg(target_os = "linux")]
async fn read_process_executable_path(pid: u32) -> Option<PathBuf> {
    // Linux exposes the real executable path via /proc
    let path = format!("/proc/{pid}/exe");
    tokio::fs::read_link(path).await.ok()
}

#[cfg(not(target_os = "linux"))]
async fn read_process_executable_path(_pid: u32) -> Option<PathBuf> {
    // Non-Linux targets skip executable path authorization
    None
}

pub(super) fn is_trusted_control_executable_path(path: &Path) -> bool {
    // Trust only sibling binaries
    let Some(trusted_dir) = trusted_control_directory() else {
        return false;
    };
    let snapshots = trusted_control_snapshots(&trusted_dir);
    is_trusted_control_executable_path_in_dir(path, &trusted_dir, snapshots)
}

pub(super) fn is_trusted_control_executable_path_in_dir(
    path: &Path,
    _trusted_dir: &Path,
    snapshots: &HashMap<String, TrustedExecutableSnapshot>,
) -> bool {
    // Canonicalize observed path first so comparisons use a stable filesystem identity
    let observed = canonicalize_best_effort(path);
    // Require an exact trusted binary name, not a suffix-alike alias
    let Some(observed_name) = observed.file_name().and_then(|name| name.to_str()) else {
        return false;
    };
    if !TRUSTED_CONTROL_EXECUTABLES.contains(&observed_name) {
        return false;
    }

    // Snapshot wins over later file swaps
    let Some(snapshot) = snapshots.get(observed_name) else {
        return false;
    };

    // Exact path match keeps trust scoped to the daemon's own sibling binary set.
    if snapshot.canonical_path != observed {
        return false;
    }

    // Recompute the live fingerprint so on-disk swaps after daemon startup are detected.
    file_fingerprint(&observed).is_some_and(|fingerprint| fingerprint == snapshot.fingerprint)
}

fn trusted_control_directory() -> Option<PathBuf> {
    // Use the daemon install dir
    let current_exe = std::env::current_exe().ok()?;
    let current_exe = canonicalize_best_effort(&current_exe);
    current_exe.parent().map(|parent| parent.to_path_buf())
}

fn trusted_control_snapshots(
    trusted_dir: &Path,
) -> &'static HashMap<String, TrustedExecutableSnapshot> {
    static SNAPSHOTS: OnceLock<HashMap<String, TrustedExecutableSnapshot>> = OnceLock::new();
    // Build the snapshot once
    SNAPSHOTS.get_or_init(|| build_trusted_control_snapshots(trusted_dir))
}

pub(super) fn build_trusted_control_snapshots(
    trusted_dir: &Path,
) -> HashMap<String, TrustedExecutableSnapshot> {
    // Read trusted files once
    let mut snapshots = HashMap::new();
    for executable in TRUSTED_CONTROL_EXECUTABLES {
        // Missing file means not trusted
        let candidate = trusted_dir.join(executable);
        if !candidate.is_file() {
            continue;
        }

        let canonical = canonicalize_best_effort(&candidate);
        let Some(fingerprint) = file_fingerprint(&canonical) else {
            continue;
        };
        snapshots.insert(
            executable.to_string(),
            TrustedExecutableSnapshot {
                canonical_path: canonical,
                fingerprint,
            },
        );
    }
    snapshots
}

fn file_fingerprint(path: &Path) -> Option<FileFingerprint> {
    let metadata = std::fs::metadata(path).ok()?;
    if !metadata.is_file() {
        return None;
    }

    // Hash the whole file
    let mut file = File::open(path).ok()?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 16 * 1024];
    loop {
        let read = file.read(&mut buf).ok()?;
        if read == 0 {
            break;
        }
        hasher.update(&buf[..read]);
    }

    Some(FileFingerprint {
        len: metadata.len(),
        sha256: hasher.finalize().into(),
    })
}

fn canonicalize_best_effort(path: &Path) -> PathBuf {
    // Fall back to the raw path
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}
