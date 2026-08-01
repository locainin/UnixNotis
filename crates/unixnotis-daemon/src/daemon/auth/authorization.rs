//! Caller authorization flow for control operations

use std::path::Path;
use std::sync::Arc;

use rustix::process::geteuid;
use tracing::warn;
use zbus::message::Header;

use crate::daemon::DaemonState;

use super::credentials::{connection_credentials, CallerCredentials};
#[cfg(target_os = "linux")]
use super::executable_trust::is_trusted_control_executable_from_fd;
#[cfg(not(target_os = "linux"))]
use super::executable_trust::is_trusted_control_executable_path;
use super::policy::{
    TRUSTED_CONTROL_EXECUTABLES, TRUSTED_INTERACTION_EXECUTABLES,
    TRUSTED_PANEL_READINESS_EXECUTABLES, TRUSTED_POPUP_READINESS_EXECUTABLES,
};
#[cfg(not(target_os = "linux"))]
use super::process_identity::read_process_executable_path;
#[cfg(target_os = "linux")]
use super::process_identity::{
    open_process_executable_from_pidfd, read_process_executable_path_from_pidfd,
};

pub(in crate::daemon) async fn authorize_control_call(
    state: &Arc<DaemonState>,
    header: &Header<'_>,
    method: &'static str,
) -> zbus::fdo::Result<()> {
    authorize_control_call_for_executables(state, header, method, &TRUSTED_CONTROL_EXECUTABLES)
        .await
}

pub(in crate::daemon) async fn authorize_panel_readiness_call(
    state: &Arc<DaemonState>,
    header: &Header<'_>,
    method: &'static str,
) -> zbus::fdo::Result<()> {
    authorize_control_call_for_executables(
        state,
        header,
        method,
        &TRUSTED_PANEL_READINESS_EXECUTABLES,
    )
    .await
}

pub(in crate::daemon) async fn authorize_interaction_call(
    state: &Arc<DaemonState>,
    header: &Header<'_>,
    method: &'static str,
) -> zbus::fdo::Result<()> {
    authorize_control_call_for_executables(state, header, method, &TRUSTED_INTERACTION_EXECUTABLES)
        .await
}

pub(in crate::daemon) async fn authorize_popup_readiness_call(
    state: &Arc<DaemonState>,
    header: &Header<'_>,
    method: &'static str,
) -> zbus::fdo::Result<()> {
    authorize_control_call_for_executables(
        state,
        header,
        method,
        &TRUSTED_POPUP_READINESS_EXECUTABLES,
    )
    .await
}

async fn authorize_control_call_for_executables(
    state: &Arc<DaemonState>,
    header: &Header<'_>,
    method: &'static str,
    allowed_executables: &[&str],
) -> zbus::fdo::Result<()> {
    // D-Bus sender identity is supplied by the bus and cannot be spoofed by payload data
    let sender = header
        .sender()
        .ok_or_else(|| zbus::fdo::Error::AccessDenied("missing sender".to_string()))?;
    let sender_name = sender.as_str().to_string();

    if state.control_owner_is_preauthorized(&sender_name) {
        // The policy is internal and production startup always selects strict verification
        return Ok(());
    }

    let bus_name = zbus::names::BusName::try_from(sender_name.as_str())
        .map_err(|_error| zbus::fdo::Error::AccessDenied("invalid sender".to_string()))?;
    // One bus reply keeps all identity fields tied to the same sender snapshot
    let credentials = connection_credentials(state.connection(), bus_name).await?;

    // Same desktop uid is required before any executable trust checks are considered
    let caller_uid = credentials
        .unix_user_id()
        .ok_or_else(|| zbus::fdo::Error::AccessDenied("caller uid is unavailable".to_string()))?;
    let expected_uid = geteuid().as_raw();
    if let Some(err) = control_owner_uid_error(caller_uid, expected_uid) {
        warn!(
            method,
            sender = %sender_name,
            uid = caller_uid,
            expected_uid,
            "rejected control caller with mismatched uid"
        );
        return Err(err);
    }

    // Same user is not enough; the executable must be one trusted UnixNotis binary
    let pid = credentials.process_id().ok_or_else(|| {
        zbus::fdo::Error::AccessDenied("caller process id is unavailable".to_string())
    })?;
    #[cfg(target_os = "linux")]
    let (exe_path, exe_fd) = {
        // Linux must use the stable process handle from the same credential snapshot
        let pidfd = required_linux_process_fd(&credentials)?;
        let exe_path = read_process_executable_path_from_pidfd(pidfd, pid);
        // Open /proc/<pid>/exe as a descriptor to fingerprint the actual file object
        // rather than a pathname that could be shadowed by a mount namespace
        let exe_fd = open_process_executable_from_pidfd(pidfd, pid);
        (exe_path, exe_fd)
    };
    #[cfg(not(target_os = "linux"))]
    let (exe_path, exe_fd) = (read_process_executable_path(pid).await, None);
    if let Some(err) = control_executable_error(
        exe_path.as_deref(),
        exe_fd.as_ref(),
        allowed_executables,
        state.trial_mode(),
    ) {
        warn!(
            method,
            sender = %sender_name,
            pid,
            executable = exe_path
                .as_ref().map_or_else(|| "unknown".to_string(), |path| path.display().to_string()),
            "rejected untrusted control caller"
        );
        return Err(err);
    }

    Ok(())
}

#[cfg(target_os = "linux")]
pub(super) fn required_linux_process_fd(
    credentials: &CallerCredentials,
) -> zbus::fdo::Result<&zbus::zvariant::OwnedFd> {
    credentials.process_fd().ok_or_else(|| {
        zbus::fdo::Error::AccessDenied(
            "caller stable process handle is unavailable on Linux".to_string(),
        )
    })
}

pub(in crate::daemon) const fn control_owner_uid_is_allowed(
    caller_uid: u32,
    expected_uid: u32,
) -> bool {
    caller_uid == expected_uid
}

pub(in crate::daemon) fn control_owner_uid_error(
    caller_uid: u32,
    expected_uid: u32,
) -> Option<zbus::fdo::Error> {
    if control_owner_uid_is_allowed(caller_uid, expected_uid) {
        return None;
    }
    Some(zbus::fdo::Error::AccessDenied(
        "caller uid is not authorized for control operation".to_string(),
    ))
}

pub(in crate::daemon) fn control_executable_is_allowed<Fd: std::os::unix::io::AsFd>(
    path: Option<&Path>,
    exe_fd: Option<&Fd>,
    allowed_executables: &[&str],
    relaxed: bool,
) -> bool {
    // Name allowlist is required; path trust is a separate check that must also pass
    let Some(path) = path else {
        return false;
    };
    let name_allowed = path
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| allowed_executables.contains(&name));
    if !name_allowed {
        return false;
    }

    // On Linux, verify the executable via its file descriptor to prevent
    // mount-namespace bypass (UNX-4-001). The path is only used for the
    // name allowlist above; the actual trust check uses the kernel file object.
    // If we cannot obtain the descriptor, fail closed.
    #[cfg(target_os = "linux")]
    {
        let Some(fd) = exe_fd else {
            return false;
        };
        is_trusted_control_executable_from_fd(fd, path, relaxed)
    }

    // Fallback for non-Linux: use path-based trust
    #[cfg(not(target_os = "linux"))]
    {
        is_trusted_control_executable_path(path, relaxed)
    }
}

pub(in crate::daemon) fn control_executable_error<Fd: std::os::unix::io::AsFd>(
    path: Option<&Path>,
    exe_fd: Option<&Fd>,
    allowed_executables: &[&str],
    relaxed: bool,
) -> Option<zbus::fdo::Error> {
    if control_executable_is_allowed(path, exe_fd, allowed_executables, relaxed) {
        return None;
    }
    Some(zbus::fdo::Error::AccessDenied(
        "caller is not authorized for control operation".to_string(),
    ))
}
