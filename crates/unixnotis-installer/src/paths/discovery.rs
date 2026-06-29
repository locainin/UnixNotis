//! Install path discovery and service-manager construction

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};

use crate::service_manager::ServiceManager;

use super::choice::ServiceManagerChoice;
use super::dirs::{
    dinit_user_dir, home_dir, runit_user_dir, runit_user_dir_candidates, s6_live_dir, s6_user_dir,
    s6_user_dir_candidates, systemd_user_dir,
};

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

    pub fn alternate_service_managers(&self) -> Vec<Result<ServiceManager>> {
        // This is used only for conflict scans; normal install still works through self.service
        ServiceManagerChoice::all()
            .into_iter()
            // Each choice can produce more than one root when an override and fallback both matter
            .flat_map(service_manager_candidates_from_choice)
            .filter_map(|manager| match manager {
                // Same backend and same artifact root is the selected install, so reinstall is valid
                Ok(manager) if manager.manages_same_backend_root(&self.service) => None,
                Ok(manager) => Some(Ok(manager)),
                // Bad optional backend paths should be visible as scan warnings
                Err(err) => Some(Err(err)),
            })
            .collect()
    }
}

fn service_manager_from_selection(
    service_manager: Option<ServiceManagerChoice>,
) -> Result<ServiceManager> {
    let choice = service_manager
        .map(Ok)
        .unwrap_or_else(service_manager_choice_from_environment)?;
    service_manager_from_choice(choice)
}

fn service_manager_from_choice(choice: ServiceManagerChoice) -> Result<ServiceManager> {
    // Keep every backend constructor in one place so new manager roots stay easy to audit
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

fn service_manager_candidates_from_choice(
    choice: ServiceManagerChoice,
) -> Vec<Result<ServiceManager>> {
    // Conflict scans inspect both selected override roots and conventional fallback roots
    match choice {
        // Systemd and dinit have one user artifact root in the current installer model
        ServiceManagerChoice::Systemd => vec![systemd_user_dir().map(ServiceManager::systemd_user)],
        ServiceManagerChoice::Dinit => vec![dinit_user_dir().map(ServiceManager::dinit_user)],
        // Runit can be redirected by project env or SVDIR while an old default service remains
        ServiceManagerChoice::Runit => runit_user_dir_candidates()
            .into_iter()
            .map(|root| root.map(ServiceManager::runit_user))
            .collect(),
        // s6 data roots need a matching live root so runtime probes still use valid commands
        ServiceManagerChoice::S6 => s6_user_dir_candidates()
            .into_iter()
            .map(|root| {
                root.and_then(|data_root| {
                    let live_root = s6_live_dir(&data_root)?;
                    Ok(ServiceManager::s6_user(data_root, live_root))
                })
            })
            .collect(),
    }
}

fn service_manager_choice_from_environment() -> Result<ServiceManagerChoice> {
    match env::var("UNIXNOTIS_SERVICE_MANAGER") {
        Ok(raw) => ServiceManagerChoice::parse(&raw),
        Err(_) => Ok(ServiceManagerChoice::Systemd),
    }
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

pub(in crate::paths) fn is_unixnotis_repo(cargo_toml: &Path) -> bool {
    let Ok(contents) = fs::read_to_string(cargo_toml) else {
        return false;
    };
    // Repo-root discovery must identify the workspace, not a member crate with a matching name
    contents.contains("[workspace]")
        && contents.contains("crates/unixnotis-daemon")
        && contents.contains("crates/unixnotis-core")
}
