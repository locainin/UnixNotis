//! Filesystem layout helpers for UnixNotis installation paths.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};

use crate::service_manager::ServiceManager;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ServiceManagerChoice {
    Systemd,
    Dinit,
    Runit,
    S6,
}

impl ServiceManagerChoice {
    pub fn parse(raw: &str) -> Result<Self> {
        match raw.trim() {
            "" | "systemd" | "systemd-user" => Ok(Self::Systemd),
            "dinit" | "dinit-user" => Ok(Self::Dinit),
            "runit" | "runit-user" => Ok(Self::Runit),
            "s6" | "s6-user" => Ok(Self::S6),
            other => Err(anyhow!("unsupported service manager '{other}'")),
        }
    }
}

pub struct InstallPaths {
    pub repo_root: PathBuf,
    pub bin_dir: PathBuf,
    pub service: ServiceManager,
}

impl InstallPaths {
    #[cfg(test)]
    pub fn discover() -> Result<Self> {
        Self::discover_with_service_manager(None)
    }

    pub fn discover_repo_root() -> Result<PathBuf> {
        // Trial mode only needs the workspace root, not install or service-manager paths
        find_repo_root()
    }

    pub fn discover_with_service_manager(
        service_manager: Option<ServiceManagerChoice>,
    ) -> Result<Self> {
        // Repo root anchors cargo metadata lookups and all local asset paths
        let repo_root = find_repo_root()?;
        // User binaries live under ~/.local/bin for install and uninstall
        let bin_dir = home_dir()?.join(".local").join("bin");
        // Backend selection stays centralized so installer actions stay manager-agnostic
        let service = service_manager_from_selection(service_manager)?;

        Ok(Self {
            repo_root,
            bin_dir,
            service,
        })
    }
}

pub fn home_dir() -> Result<PathBuf> {
    let home = env::var("HOME").map_err(|_| anyhow!("HOME is not set"))?;
    Ok(PathBuf::from(home))
}

fn xdg_config_home() -> Option<PathBuf> {
    let raw = env::var("XDG_CONFIG_HOME").ok()?;
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    let path = PathBuf::from(trimmed);
    if path.is_absolute() {
        Some(path)
    } else {
        None
    }
}

fn systemd_user_dir() -> Result<PathBuf> {
    if let Some(base) = xdg_config_home() {
        Ok(base.join("systemd").join("user"))
    } else {
        Ok(home_dir()?.join(".config").join("systemd").join("user"))
    }
}

fn dinit_user_dir() -> Result<PathBuf> {
    if let Some(base) = xdg_config_home() {
        Ok(base.join("dinit.d"))
    } else {
        Ok(home_dir()?.join(".config").join("dinit.d"))
    }
}

fn runit_user_dir() -> Result<PathBuf> {
    if let Some(path) = absolute_env_path("UNIXNOTIS_RUNIT_SERVICE_DIR")? {
        return Ok(path);
    }
    if let Some(path) = absolute_env_path("SVDIR")? {
        return Ok(path);
    }
    Ok(home_dir()?.join(".config").join("service"))
}

fn s6_user_dir() -> Result<PathBuf> {
    if let Some(path) = absolute_env_path("UNIXNOTIS_S6_DATA_DIR")? {
        // Custom roots are safe now that UnixNotis compiles the s6-rc database directly
        return Ok(path);
    }
    // Artix documents local user s6 data under ~/.local/share/s6
    Ok(home_dir()?.join(".local").join("share").join("s6"))
}

fn s6_live_dir(data_root: &Path) -> Result<PathBuf> {
    if let Some(path) = absolute_env_path("UNIXNOTIS_S6RC_LIVE_DIR")? {
        // Explicit live roots are for testers and advanced users who already know their tree
        return Ok(path);
    }
    let user = env::var("USER").map_err(|_| anyhow!("USER is not set"))?;
    let integrated = PathBuf::from("/run").join(&user).join("s6-rc");
    if path_is_directory_or_symlink_to_directory(&integrated) {
        // Artix integrated local supervision wires the user s6-rc tree under /run/$USER
        // s6-rc-init normally exposes this live path as a symlink to a real live directory
        return Ok(integrated);
    }
    let standalone = PathBuf::from("/tmp").join(&user).join("s6-rc");
    if path_is_plain_directory(&standalone) {
        // Artix standalone local supervision uses /tmp/$USER/s6-rc in its documented setup
        return Ok(standalone);
    }
    let local = data_root.join("rc").join("live");
    if path_is_directory_or_symlink_to_directory(&local) {
        // Test and custom layouts can keep a live tree beside the compiled database root
        // Keep the symlink name because s6-rc-update expects the original live argument
        return Ok(local);
    }
    // Return the integrated path so readiness can show the normal setup hint
    Ok(integrated)
}

fn path_is_directory_or_symlink_to_directory(path: &Path) -> bool {
    fs::metadata(path)
        // s6 live roots are expected to be symlinks that point at the current live tree
        .map(|metadata| metadata.is_dir())
        .unwrap_or(false)
}

fn path_is_plain_directory(path: &Path) -> bool {
    fs::symlink_metadata(path)
        // Auto-detected /tmp roots must not follow symlinks into surprising locations
        .map(|metadata| metadata.file_type().is_dir())
        .unwrap_or(false)
}

fn absolute_env_path(name: &str) -> Result<Option<PathBuf>> {
    let Ok(raw) = env::var(name) else {
        return Ok(None);
    };
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    let path = PathBuf::from(trimmed);
    if !path.is_absolute() {
        return Err(anyhow!("{name} must be an absolute path"));
    }
    Ok(Some(path))
}

fn service_manager_from_selection(
    service_manager: Option<ServiceManagerChoice>,
) -> Result<ServiceManager> {
    let choice = service_manager
        .map(Ok)
        .unwrap_or_else(service_manager_choice_from_environment)?;
    match choice {
        ServiceManagerChoice::Systemd => Ok(ServiceManager::systemd_user(systemd_user_dir()?)),
        ServiceManagerChoice::Dinit => Ok(ServiceManager::dinit_user(dinit_user_dir()?)),
        ServiceManagerChoice::Runit => Ok(ServiceManager::runit_user(runit_user_dir()?)),
        ServiceManagerChoice::S6 => {
            let data_root = s6_user_dir()?;
            let live_root = s6_live_dir(&data_root)?;
            Ok(ServiceManager::s6_user(data_root, live_root))
        }
    }
}

fn service_manager_choice_from_environment() -> Result<ServiceManagerChoice> {
    match env::var("UNIXNOTIS_SERVICE_MANAGER") {
        Ok(raw) => ServiceManagerChoice::parse(&raw),
        Err(_) => Ok(ServiceManagerChoice::Systemd),
    }
}

pub fn format_with_home(path: &Path) -> String {
    if let Ok(home) = home_dir() {
        if let Ok(stripped) = path.strip_prefix(&home) {
            let mut rendered = PathBuf::from("$HOME");
            rendered.push(stripped);
            return rendered.display().to_string();
        }
    }
    path.display().to_string()
}

fn find_repo_root() -> Result<PathBuf> {
    if let Ok(root) = env::var("UNIXNOTIS_REPO_ROOT") {
        let root_path = PathBuf::from(root);
        let cargo = root_path.join("Cargo.toml");
        // Keep the override strict so install does not wander into the wrong workspace
        if cargo.is_file() && is_unixnotis_repo(&cargo) {
            return Ok(root_path);
        }
    }

    let mut dir = env::current_dir()?;
    loop {
        let cargo = dir.join("Cargo.toml");
        // Walk upward until the real workspace root is found
        if cargo.is_file() && is_unixnotis_repo(&cargo) {
            return Ok(dir);
        }
        if !dir.pop() {
            break;
        }
    }

    Err(anyhow!(
        "repository root not found (set UNIXNOTIS_REPO_ROOT or run from UnixNotis repo)"
    ))
}

fn is_unixnotis_repo(cargo_toml: &Path) -> bool {
    let Ok(contents) = fs::read_to_string(cargo_toml) else {
        return false;
    };
    let markers = [
        "crates/unixnotis-daemon",
        "crates/unixnotis-core",
        "name = \"unixnotis-daemon\"",
    ];
    markers.iter().any(|marker| contents.contains(marker))
}

#[cfg(test)]
#[path = "tests/paths.rs"]
mod tests;
