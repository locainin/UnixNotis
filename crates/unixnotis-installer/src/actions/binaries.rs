//! Shared logic for resolving which binaries the installer manages.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};

use crate::managed_binaries::{is_managed_binary_name, validate_managed_binary_names};
use crate::paths::InstallPaths;
use crate::toolchain::{cargo_command, resolve_cargo};

pub(super) fn resolve_install_binaries(paths: &InstallPaths) -> Result<Vec<String>> {
    let cargo = if paths.is_release_archive() {
        None
    } else {
        Some(resolve_cargo()?)
    };
    resolve_install_binaries_with_cargo(paths, cargo.as_deref())
}

pub(super) fn resolve_install_binaries_with_cargo(
    paths: &InstallPaths,
    cargo: Option<&Path>,
) -> Result<Vec<String>> {
    // Prefer the installer metadata list when it is present.
    let metadata_list = load_install_binaries_from_metadata(paths)?;
    if !metadata_list.is_empty() {
        // Validate against cargo metadata when available to catch stale entries.
        if let Some(cargo) = cargo {
            // An empty Cargo inventory is an error and cannot widen the declared list
            let available = load_install_binaries_from_cargo_metadata(paths, cargo)?;
            let missing = metadata_list
                .iter()
                .filter(|name| !available.contains(*name))
                .cloned()
                .collect::<Vec<_>>();
            if !missing.is_empty() {
                return Err(anyhow!(
                    "installer metadata lists binaries missing from workspace: {}",
                    missing.join(", ")
                ));
            }
        }
        return Ok(metadata_list);
    }

    // Fall back to cargo metadata when no installer list is declared.
    if let Some(cargo) = cargo {
        return load_install_binaries_from_cargo_metadata(paths, cargo)
            .with_context(|| "no installable binaries discovered from cargo metadata");
    }

    // Install should stop here instead of guessing a binary list
    Err(anyhow!(
        "no installable binaries discovered from installer metadata or cargo metadata"
    ))
}

pub(super) fn resolve_target_directory(paths: &InstallPaths) -> Result<PathBuf> {
    if paths.is_release_archive() {
        // Release archives already contain built binaries under their local bin directory
        return Ok(paths.repo_root.clone());
    }
    let cargo = resolve_cargo()?;
    resolve_target_directory_with_cargo(paths, &cargo)
}

pub(super) fn resolve_target_directory_with_cargo(
    paths: &InstallPaths,
    cargo: &Path,
) -> Result<PathBuf> {
    let metadata = load_cargo_metadata(paths, cargo)?;
    Ok(metadata.target_directory)
}

pub(super) fn resolve_install_binaries_best_effort(
    paths: &InstallPaths,
) -> (Vec<String>, Option<String>) {
    // Best-effort resolution keeps uninstall working even if workspace metadata is broken.
    match resolve_install_binaries(paths) {
        Ok(binaries) => (binaries, None),
        Err(err) => {
            let discovered = discover_installed_binaries(paths);
            if !discovered.is_empty() {
                return (discovered, Some(err.to_string()));
            }
            (legacy_binaries(), Some(err.to_string()))
        }
    }
}

fn legacy_binaries() -> Vec<String> {
    vec![
        "unixnotis-daemon".to_string(),
        "unixnotis-popups".to_string(),
        "unixnotis-center".to_string(),
        "unixnotis-svg-renderer".to_string(),
        "unixnotis-css-validate".to_string(),
        "noticenterctl".to_string(),
    ]
}

fn load_install_binaries_from_metadata(paths: &InstallPaths) -> Result<Vec<String>> {
    if paths.is_release_archive() {
        return load_install_binaries_from_release_manifest(paths);
    }

    // Read the root Cargo.toml and extract the installer metadata list if present.
    let cargo_path = paths.repo_root.join("Cargo.toml");
    let contents =
        fs::read_to_string(&cargo_path).with_context(|| "failed to read workspace Cargo.toml")?;
    parse_install_binaries_metadata(&contents)
}

fn load_install_binaries_from_release_manifest(paths: &InstallPaths) -> Result<Vec<String>> {
    // Release archives do not include Cargo metadata, so the manifest is the source of truth
    let contents = fs::read_to_string(paths.release_manifest_path())
        .with_context(|| "failed to read UnixNotis release manifest")?;
    parse_release_manifest_binaries(&contents)
}

fn parse_release_manifest_binaries(contents: &str) -> Result<Vec<String>> {
    // The release manifest intentionally stores only deployable runtime binary names
    let manifest: ReleaseManifest = serde_json::from_str(contents)
        .with_context(|| "failed to parse UnixNotis release manifest")?;
    // Release data is external to the binary, so accept only known runtime targets
    validate_managed_binary_names(manifest.binaries)
}

fn parse_install_binaries_metadata(contents: &str) -> Result<Vec<String>> {
    // Deserialize a minimal schema so the metadata stays readable and future-safe.
    let root: WorkspaceCargoToml =
        toml::from_str(contents).with_context(|| "failed to parse workspace Cargo.toml")?;
    let array = root
        .workspace
        .and_then(|workspace| workspace.metadata)
        .and_then(|metadata| metadata.unixnotis)
        .and_then(|unixnotis| unixnotis.installer)
        .and_then(|installer| installer.binaries);

    // A missing list enables Cargo discovery while every explicit list uses the common policy
    array.map_or_else(|| Ok(Vec::new()), validate_managed_binary_names)
}

fn discover_installed_binaries(paths: &InstallPaths) -> Vec<String> {
    // Best-effort scan of the install bin directory to keep uninstall resilient.
    // Only UnixNotis-prefixed binaries are collected to avoid touching unrelated tools.
    let Ok(entries) = fs::read_dir(&paths.bin_dir) else {
        return Vec::new();
    };

    let mut candidates = BTreeSet::new();
    for entry in entries.flatten() {
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if !file_type.is_file() {
            continue;
        }
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        // Prefix matches are too broad for a path that uninstall may remove
        if is_managed_binary_name(name) {
            candidates.insert(name.to_string());
        }
    }

    candidates.into_iter().collect()
}

#[derive(serde::Deserialize)]
struct WorkspaceCargoToml {
    workspace: Option<WorkspaceSection>,
}

#[derive(serde::Deserialize)]
struct WorkspaceSection {
    metadata: Option<WorkspaceMetadata>,
}

#[derive(serde::Deserialize)]
struct WorkspaceMetadata {
    unixnotis: Option<UnixnotisMetadata>,
}

#[derive(serde::Deserialize)]
struct UnixnotisMetadata {
    installer: Option<InstallerMetadata>,
}

#[derive(serde::Deserialize)]
struct InstallerMetadata {
    binaries: Option<Vec<String>>,
}

#[derive(serde::Deserialize)]
struct ReleaseManifest {
    binaries: Vec<String>,
}

fn load_install_binaries_from_cargo_metadata(
    paths: &InstallPaths,
    cargo: &Path,
) -> Result<Vec<String>> {
    let metadata = load_cargo_metadata(paths, cargo)?;
    extract_bins_from_metadata(&metadata)
}

fn load_cargo_metadata(paths: &InstallPaths, cargo: &Path) -> Result<CargoMetadata> {
    // cargo metadata is the most robust source of workspace targets.
    let output = cargo_command(cargo)?
        .args(["metadata", "--no-deps", "--format-version", "1"])
        .current_dir(&paths.repo_root)
        .output()
        .with_context(|| "failed to run cargo metadata")?;

    if !output.status.success() {
        return Err(anyhow!(
            "cargo metadata exited with status {}",
            output.status
        ));
    }

    serde_json::from_slice(&output.stdout).with_context(|| "failed to parse cargo metadata")
}

fn extract_bins_from_metadata(metadata: &CargoMetadata) -> Result<Vec<String>> {
    let mut binaries = BTreeSet::new();
    for package in &metadata.packages {
        for target in &package.targets {
            if target.kind.iter().any(|kind| kind == "bin") && is_managed_binary_name(&target.name)
            {
                // Cargo also reports internal helper binaries that are not installed
                binaries.insert(target.name.clone());
            }
        }
    }
    // Only allowlisted targets can reach install or uninstall planning
    validate_managed_binary_names(binaries.into_iter().collect())
}

#[derive(serde::Deserialize)]
struct CargoMetadata {
    target_directory: PathBuf,
    packages: Vec<CargoPackage>,
}

#[derive(serde::Deserialize)]
struct CargoPackage {
    targets: Vec<CargoTarget>,
}

#[derive(serde::Deserialize)]
struct CargoTarget {
    name: String,
    kind: Vec<String>,
}

#[cfg(test)]
#[path = "tests/binaries.rs"]
mod tests;
