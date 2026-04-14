//! Shared logic for resolving which binaries the installer manages.

use std::collections::{BTreeSet, HashSet};
use std::fs;
use std::path::PathBuf;
use std::process::Command;

use anyhow::{anyhow, Context, Result};

use crate::paths::InstallPaths;
use unixnotis_core::program_in_path;

pub(super) fn resolve_install_binaries(paths: &InstallPaths) -> Result<Vec<String>> {
    // Prefer the installer metadata list when it is present.
    let metadata_list = load_install_binaries_from_metadata(paths)?;
    let cargo_available = program_in_path("cargo");
    if !metadata_list.is_empty() {
        // Validate against cargo metadata when available to catch stale entries.
        if cargo_available {
            let available = load_install_binaries_from_cargo_metadata(paths)?;
            if !available.is_empty() {
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
        }
        return Ok(metadata_list);
    }

    // Fall back to cargo metadata when no installer list is declared.
    if cargo_available {
        let metadata = load_install_binaries_from_cargo_metadata(paths)?;
        if !metadata.is_empty() {
            return Ok(metadata);
        }
    }

    // Install should stop here instead of guessing a binary list
    Err(anyhow!(
        "no installable binaries discovered from installer metadata or cargo metadata"
    ))
}

pub(super) fn resolve_target_directory(paths: &InstallPaths) -> Result<PathBuf> {
    let metadata = load_cargo_metadata(paths)?;
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
        "noticenterctl".to_string(),
    ]
}

fn load_install_binaries_from_metadata(paths: &InstallPaths) -> Result<Vec<String>> {
    // Read the root Cargo.toml and extract the installer metadata list if present.
    let cargo_path = paths.repo_root.join("Cargo.toml");
    let contents =
        fs::read_to_string(&cargo_path).with_context(|| "failed to read workspace Cargo.toml")?;
    parse_install_binaries_metadata(&contents)
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
        .and_then(|installer| installer.binaries)
        .unwrap_or_default();

    let mut seen = HashSet::new();
    let mut binaries = Vec::new();
    for name in array {
        let name = name.trim();
        if name.is_empty() {
            continue;
        }
        if seen.insert(name.to_string()) {
            binaries.push(name.to_string());
        }
    }
    Ok(binaries)
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
        let name = name.to_string_lossy();
        if name == "noticenterctl" || name.starts_with("unixnotis-") {
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

fn load_install_binaries_from_cargo_metadata(paths: &InstallPaths) -> Result<Vec<String>> {
    let metadata = load_cargo_metadata(paths)?;
    Ok(extract_bins_from_metadata(&metadata))
}

fn load_cargo_metadata(paths: &InstallPaths) -> Result<CargoMetadata> {
    // cargo metadata is the most robust source of workspace targets.
    let output = Command::new("cargo")
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

fn extract_bins_from_metadata(metadata: &CargoMetadata) -> Vec<String> {
    let mut binaries = BTreeSet::new();
    for package in &metadata.packages {
        for target in &package.targets {
            if target.kind.iter().any(|kind| kind == "bin") {
                if target.name == "unixnotis-installer" {
                    continue;
                }
                binaries.insert(target.name.clone());
            }
        }
    }
    binaries.into_iter().collect()
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
mod tests {
    use super::{extract_bins_from_metadata, parse_install_binaries_metadata, CargoMetadata};

    #[test]
    fn parse_install_binaries_metadata_reads_entries() {
        // Ensures installer metadata is read and de-duplicated in declaration order.
        let input = r#"
[workspace.metadata.unixnotis.installer]
binaries = ["unixnotis-daemon", "noticenterctl", "unixnotis-daemon"]
"#;
        let binaries = parse_install_binaries_metadata(input).expect("valid metadata");
        assert_eq!(
            binaries,
            vec!["unixnotis-daemon".to_string(), "noticenterctl".to_string()]
        );
    }

    #[test]
    fn parse_install_binaries_metadata_handles_missing_table() {
        // Confirms missing metadata simply yields an empty list.
        let input = r#"
[workspace]
members = ["crates/unixnotis-daemon"]
"#;
        let binaries = parse_install_binaries_metadata(input).expect("valid metadata");
        assert!(binaries.is_empty());
    }

    #[test]
    fn parse_install_binaries_metadata_handles_empty_entries() {
        let input = r#"
[workspace.metadata.unixnotis.installer]
binaries = ["unixnotis-daemon", "  ", ""]
"#;
        let binaries = parse_install_binaries_metadata(input).expect("valid metadata");
        assert_eq!(binaries, vec!["unixnotis-daemon".to_string()]);
    }

    #[test]
    fn extract_bins_from_cargo_metadata() {
        let input = r#"
{
  "target_directory": "target",
  "packages": [
    {
      "targets": [
        { "name": "unixnotis-daemon", "kind": ["bin"] },
        { "name": "unixnotis-installer", "kind": ["bin"] },
        { "name": "unixnotis-core", "kind": ["lib"] }
      ]
    }
  ]
}
"#;
        let metadata: CargoMetadata = serde_json::from_str(input).expect("metadata");
        let binaries = extract_bins_from_metadata(&metadata);
        assert_eq!(binaries, vec!["unixnotis-daemon".to_string()]);
    }
}
