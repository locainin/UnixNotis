//! Install path discovery and service-manager construction

use std::collections::BTreeSet;
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

pub const RELEASE_MANIFEST_FILE: &str = "unixnotis-release.json";
pub const RELEASE_BIN_DIR: &str = "bin";

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

    pub fn is_release_archive(&self) -> bool {
        // Release archives carry a manifest beside the installer instead of a workspace Cargo.toml
        self.release_manifest_path().is_file()
    }

    pub fn release_manifest_path(&self) -> PathBuf {
        self.repo_root.join(RELEASE_MANIFEST_FILE)
    }

    pub fn release_binary_dir(&self) -> PathBuf {
        self.repo_root.join(RELEASE_BIN_DIR)
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
    if let Ok(root) = env::var("UNIXNOTIS_RELEASE_ROOT") {
        // Manual release testing can point the installer at an unpacked archive
        let root_path = PathBuf::from(root);
        // Validate the manifest and bundled binaries before trusting the override
        if is_unixnotis_release_archive(&root_path) {
            return Ok(root_path);
        }
    }

    if let Some(root) = find_release_root_from_current_exe() {
        // Downloaded archives should resolve here before the source checkout walk below
        return Ok(root);
    }

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
        "repository root or release archive not found (set UNIXNOTIS_REPO_ROOT, set UNIXNOTIS_RELEASE_ROOT, or run from UnixNotis repo/release)"
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

fn find_release_root_from_current_exe() -> Option<PathBuf> {
    // Installed tarballs run the installer from the archive root, next to the manifest
    let exe = env::current_exe().ok()?;
    let root = exe.parent()?.to_path_buf();
    // This check prevents a random copied installer from pretending to be a full release
    is_unixnotis_release_archive(&root).then_some(root)
}

pub(in crate::paths) fn is_unixnotis_release_archive(root: &Path) -> bool {
    // A valid release archive must include the manifest file at the expected root location.
    let manifest = root.join(RELEASE_MANIFEST_FILE);

    // If the manifest cannot be read, the directory cannot be treated as a release archive.
    let Ok(contents) = fs::read_to_string(manifest) else {
        return false;
    };

    // The manifest must also match the expected JSON shape before any archive contents are trusted.
    let Ok(manifest) = serde_json::from_str::<ReleaseArchiveManifest>(&contents) else {
        return false;
    };

    // Runtime binaries are expected to live under the archive's bin directory.
    let release_bin_dir = root.join(RELEASE_BIN_DIR);

    // The archive layout is intentionally simple: installer at root, runtime tools in bin
    if !release_bin_dir.is_dir() {
        return false;
    }

    // Normalize manifest binary names by trimming whitespace and collecting into a set.
    // The set removes duplicates so each listed binary only needs to be checked once.
    let names = manifest
        .binaries
        .into_iter()
        .map(|name| name.trim().to_string())
        .collect::<BTreeSet<_>>();

    // An archive with no declared binaries is incomplete, even if the bin directory exists.
    if names.is_empty() {
        return false;
    }

    // Every declared binary must have a safe filename and exist as a regular file in bin.
    names
        .iter()
        .all(|binary| is_release_binary_name(binary) && release_bin_dir.join(binary).is_file())
}

fn is_release_binary_name(binary: &str) -> bool {
    // Only allow plain file names. Empty names, current/parent directory references,
    // and path separators are rejected to prevent escaping the release bin directory.
    !binary.is_empty()
        && binary != "."
        && binary != ".."
        && !binary.contains('/')
        && !binary.contains('\\')
}

#[derive(serde::Deserialize)]
struct ReleaseArchiveManifest {
    // The manifest declares the runtime binary filenames expected inside RELEASE_BIN_DIR.
    binaries: Vec<String>,
}
