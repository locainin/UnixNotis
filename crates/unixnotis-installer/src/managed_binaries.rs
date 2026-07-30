//! Names the installer may copy or remove

use std::collections::HashSet;
use std::path::{Component, Path};

use anyhow::{anyhow, Result};

// Every discovery source shares this list so source and archive installs have one policy
const SUPPORTED_MANAGED_BINARIES: &[&str] = &[
    "unixnotis-daemon",
    "unixnotis-popups",
    "unixnotis-center",
    "unixnotis-svg-renderer",
    "unixnotis-css-validate",
    "noticenterctl",
];

pub fn is_managed_binary_name(name: &str) -> bool {
    SUPPORTED_MANAGED_BINARIES.contains(&name)
}

pub fn validate_managed_binary_names(names: Vec<String>) -> Result<Vec<String>> {
    let mut seen = HashSet::new();
    let mut binaries = Vec::with_capacity(names.len());

    for raw_name in names {
        let name = raw_name.trim();
        // A single normal component cannot replace or walk above the managed bin directory
        let mut components = Path::new(name).components();
        let is_single_component =
            matches!(components.next(), Some(Component::Normal(_))) && components.next().is_none();
        if !is_single_component {
            return Err(anyhow!(
                "managed binary list contains an invalid path: {raw_name:?}"
            ));
        }
        // Unknown names are never inferred from a prefix because uninstall removes files
        if !is_managed_binary_name(name) {
            return Err(anyhow!(
                "managed binary list contains an unsupported name: {raw_name:?}"
            ));
        }

        if seen.insert(name.to_string()) {
            // First mention wins so plans and logs retain their declared order
            binaries.push(name.to_string());
        }
    }

    if binaries.is_empty() {
        return Err(anyhow!(
            "managed binary list does not contain any supported binaries"
        ));
    }

    Ok(binaries)
}

#[cfg(test)]
#[path = "tests/managed_binaries.rs"]
mod tests;
