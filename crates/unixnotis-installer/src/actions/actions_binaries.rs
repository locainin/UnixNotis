//! Shared logic for resolving which binaries the installer manages.

use std::collections::{BTreeSet, HashSet};
use std::fs;
use std::process::Command;

use anyhow::{anyhow, Context, Result};

use crate::paths::InstallPaths;
use unixnotis_core::program_in_path;

pub(super) fn resolve_install_binaries(paths: &InstallPaths) -> Result<Vec<String>> {
    // Prefer explicit installer metadata as the source of truth.
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

    // Last-resort fallback retains legacy behavior when discovery yields nothing.
    Ok(legacy_binaries())
}

pub(super) fn resolve_install_binaries_best_effort(
    paths: &InstallPaths,
) -> (Vec<String>, Option<String>) {
    // Best-effort resolution keeps uninstall working even if workspace metadata is broken.
    match resolve_install_binaries(paths) {
        Ok(binaries) => (binaries, None),
        Err(err) => (legacy_binaries(), Some(err.to_string())),
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
    let root: toml::Value =
        toml::from_str(&contents).with_context(|| "failed to parse workspace Cargo.toml")?;
    Ok(parse_install_binaries_metadata(&root))
}

fn parse_install_binaries_metadata(root: &toml::Value) -> Vec<String> {
    // Walk workspace.metadata.unixnotis.installer.binaries and preserve declaration order.
    let Some(array) = root
        .get("workspace")
        .and_then(|value| value.get("metadata"))
        .and_then(|value| value.get("unixnotis"))
        .and_then(|value| value.get("installer"))
        .and_then(|value| value.get("binaries"))
        .and_then(|value| value.as_array())
    else {
        return Vec::new();
    };

    let mut seen = HashSet::new();
    let mut binaries = Vec::new();
    for entry in array {
        let Some(name) = entry.as_str() else {
            continue;
        };
        let name = name.trim();
        if name.is_empty() {
            continue;
        }
        if seen.insert(name.to_string()) {
            binaries.push(name.to_string());
        }
    }
    binaries
}

fn load_install_binaries_from_cargo_metadata(paths: &InstallPaths) -> Result<Vec<String>> {
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

    let metadata: CargoMetadata =
        serde_json::from_slice(&output.stdout).with_context(|| "failed to parse cargo metadata")?;

    Ok(extract_bins_from_metadata(&metadata))
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
        let value: toml::Value = toml::from_str(input).expect("valid toml");
        let binaries = parse_install_binaries_metadata(&value);
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
        let value: toml::Value = toml::from_str(input).expect("valid toml");
        let binaries = parse_install_binaries_metadata(&value);
        assert!(binaries.is_empty());
    }

    #[test]
    fn extract_bins_from_cargo_metadata() {
        let input = r#"
{
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
