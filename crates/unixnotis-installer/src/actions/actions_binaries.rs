//! Shared logic for resolving which binaries the installer manages.

use std::collections::{BTreeSet, HashSet};
use std::fs;
use std::path::Path;

use anyhow::{Context, Result};

use crate::paths::InstallPaths;

pub(super) fn resolve_install_binaries(paths: &InstallPaths) -> Result<Vec<String>> {
    // Prefer workspace metadata so the installer and workspace stay in sync.
    let metadata = load_install_binaries_from_metadata(paths)?;
    if !metadata.is_empty() {
        return Ok(metadata);
    }

    // Fall back to workspace discovery for older checkouts without installer metadata.
    let discovered = discover_workspace_binaries(paths)?;
    if !discovered.is_empty() {
        return Ok(discovered);
    }

    // Last-resort fallback retains legacy behavior when discovery yields nothing.
    Ok(vec![
        "unixnotis-daemon".to_string(),
        "unixnotis-popups".to_string(),
        "unixnotis-center".to_string(),
        "noticenterctl".to_string(),
    ])
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

fn discover_workspace_binaries(paths: &InstallPaths) -> Result<Vec<String>> {
    // Fall back to scanning workspace members when metadata is unavailable.
    let cargo_path = paths.repo_root.join("Cargo.toml");
    let contents =
        fs::read_to_string(&cargo_path).with_context(|| "failed to read workspace Cargo.toml")?;
    let root: toml::Value =
        toml::from_str(&contents).with_context(|| "failed to parse workspace Cargo.toml")?;

    let members = root
        .get("workspace")
        .and_then(|value| value.get("members"))
        .and_then(|value| value.as_array())
        .map(|array| {
            array
                .iter()
                .filter_map(|entry| entry.as_str())
                .map(|member| paths.repo_root.join(member))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    let mut binaries = BTreeSet::new();
    for member in members {
        if let Ok(names) = discover_crate_binaries(&member) {
            for name in names {
                // Avoid auto-installing the installer itself unless explicitly configured.
                if name == "unixnotis-installer" {
                    continue;
                }
                binaries.insert(name);
            }
        }
    }

    Ok(binaries.into_iter().collect())
}

fn discover_crate_binaries(member_root: &Path) -> Result<Vec<String>> {
    // Parse the crate Cargo.toml to discover explicit [[bin]] entries first.
    let cargo_path = member_root.join("Cargo.toml");
    let contents =
        fs::read_to_string(&cargo_path).with_context(|| "failed to read crate Cargo.toml")?;
    let value: toml::Value =
        toml::from_str(&contents).with_context(|| "failed to parse crate Cargo.toml")?;

    let mut binaries = Vec::new();
    if let Some(bins) = value.get("bin").and_then(|bin| bin.as_array()) {
        for bin in bins {
            if let Some(name) = bin.get("name").and_then(|name| name.as_str()) {
                if !name.trim().is_empty() {
                    binaries.push(name.to_string());
                }
            }
        }
    }

    if !binaries.is_empty() {
        return Ok(binaries);
    }

    // Fall back to the package name when a src/main.rs exists.
    let has_main = member_root.join("src").join("main.rs").is_file();
    let package_name = value
        .get("package")
        .and_then(|package| package.get("name"))
        .and_then(|name| name.as_str());
    if has_main {
        if let Some(name) = package_name {
            if !name.trim().is_empty() {
                binaries.push(name.to_string());
            }
        }
    }

    Ok(binaries)
}

#[cfg(test)]
mod tests {
    use super::parse_install_binaries_metadata;

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
}
