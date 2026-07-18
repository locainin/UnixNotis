//! Service-manager artifact and runtime-path resolution

use std::env;
use std::path::{Path, PathBuf};

use thiserror::Error;

use super::ServiceManagerKind;

/// Resolved paths needed to inspect one service-manager installation
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ServiceManagerPaths {
    /// Selected backend
    pub kind: ServiceManagerKind,
    /// Root containing installer-managed service artifacts
    pub artifact_root: PathBuf,
    /// Runtime supervision tree used only by s6-rc
    pub live_root: Option<PathBuf>,
}

/// Failure while resolving shared service-manager paths
#[derive(Debug, Error)]
pub enum ServiceManagerPathError {
    /// HOME is required for default user roots
    #[error("HOME is not set")]
    MissingHome,
    /// USER is required for s6-rc runtime roots
    #[error("USER is not set")]
    MissingUser,
    /// Explicit path overrides must not depend on the working directory
    #[error("{0} must be an absolute path")]
    RelativeOverride(&'static str),
    /// The requested manager name is not supported
    #[error("unsupported service manager '{0}'")]
    Unsupported(String),
}

/// Resolve the selected manager using the same environment contract as the installer
///
/// # Errors
///
/// Returns an error for invalid selection or path overrides
pub fn resolve_service_manager_paths(
    kind: ServiceManagerKind,
) -> Result<ServiceManagerPaths, ServiceManagerPathError> {
    let artifact_root = match kind {
        ServiceManagerKind::Systemd => systemd_user_dir()?,
        ServiceManagerKind::Dinit => dinit_user_dir()?,
        ServiceManagerKind::Runit => runit_user_dir()?,
        ServiceManagerKind::S6 => s6_user_dir()?,
    };
    // Only s6-rc separates its compiled database from its live supervision tree
    let live_root = if kind == ServiceManagerKind::S6 {
        Some(s6_live_dir(&artifact_root)?)
    } else {
        None
    };
    Ok(ServiceManagerPaths {
        kind,
        artifact_root,
        live_root,
    })
}

/// Resolve the systemd user unit directory
///
/// # Errors
///
/// Returns an error when neither an absolute XDG config root nor HOME can resolve
pub fn systemd_user_dir() -> Result<PathBuf, ServiceManagerPathError> {
    Ok(config_home()?.join("systemd").join("user"))
}

/// Resolve the dinit user service directory
///
/// # Errors
///
/// Returns an error when neither an absolute XDG config root nor HOME can resolve
pub fn dinit_user_dir() -> Result<PathBuf, ServiceManagerPathError> {
    Ok(config_home()?.join("dinit.d"))
}

/// Resolve the selected runit supervision root
///
/// # Errors
///
/// Returns an error for relative overrides or missing HOME
pub fn runit_user_dir() -> Result<PathBuf, ServiceManagerPathError> {
    if let Some(path) = absolute_env_path("UNIXNOTIS_RUNIT_SERVICE_DIR")? {
        return Ok(path);
    }
    if let Some(path) = absolute_env_path("SVDIR")? {
        return Ok(path);
    }
    Ok(home_dir()?.join(".config").join("service"))
}

/// Resolve the selected s6-rc source and database root
///
/// # Errors
///
/// Returns an error for a relative override or missing HOME
pub fn s6_user_dir() -> Result<PathBuf, ServiceManagerPathError> {
    if let Some(path) = absolute_env_path("UNIXNOTIS_S6_DATA_DIR")? {
        return Ok(path);
    }
    Ok(home_dir()?.join(".local").join("share").join("s6"))
}

/// Resolve the selected s6-rc live runtime directory
///
/// # Errors
///
/// Returns an error for a relative override or missing USER
pub fn s6_live_dir(data_root: &Path) -> Result<PathBuf, ServiceManagerPathError> {
    if let Some(path) = absolute_env_path("UNIXNOTIS_S6RC_LIVE_DIR")? {
        return Ok(path);
    }
    let user = env::var("USER").map_err(|_error| ServiceManagerPathError::MissingUser)?;
    // Integrated installations conventionally expose the live tree below /run
    let integrated = PathBuf::from("/run").join(&user).join("s6-rc");
    if directory_or_directory_symlink(&integrated) {
        return Ok(integrated);
    }
    // Standalone user supervisors commonly keep their live tree below /tmp
    let standalone = PathBuf::from("/tmp").join(&user).join("s6-rc");
    if plain_directory(&standalone) {
        return Ok(standalone);
    }
    let local = data_root.join("rc").join("live");
    if directory_or_directory_symlink(&local) {
        return Ok(local);
    }
    // Preserve the installer contract when no live tree exists yet
    Ok(integrated)
}

fn config_home() -> Result<PathBuf, ServiceManagerPathError> {
    if let Some(path) = absolute_xdg_path("XDG_CONFIG_HOME") {
        return Ok(path);
    }
    Ok(home_dir()?.join(".config"))
}

fn absolute_xdg_path(name: &'static str) -> Option<PathBuf> {
    let raw = env::var_os(name)?;
    let path = PathBuf::from(raw);

    // XDG requires absolute paths, so invalid values fall back to the standard home path
    path.is_absolute().then_some(path)
}

fn home_dir() -> Result<PathBuf, ServiceManagerPathError> {
    env::var_os("HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .ok_or(ServiceManagerPathError::MissingHome)
}

fn absolute_env_path(name: &'static str) -> Result<Option<PathBuf>, ServiceManagerPathError> {
    let Some(raw) = env::var_os(name) else {
        return Ok(None);
    };
    if raw.is_empty() {
        return Ok(None);
    }
    let path = PathBuf::from(raw);
    if !path.is_absolute() {
        return Err(ServiceManagerPathError::RelativeOverride(name));
    }
    Ok(Some(path))
}

fn directory_or_directory_symlink(path: &Path) -> bool {
    if path.is_dir() {
        return true;
    }
    std::fs::symlink_metadata(path).is_ok_and(|metadata| metadata.file_type().is_symlink())
        && std::fs::metadata(path).is_ok_and(|metadata| metadata.is_dir())
}

fn plain_directory(path: &Path) -> bool {
    std::fs::symlink_metadata(path).is_ok_and(|metadata| metadata.file_type().is_dir())
}
