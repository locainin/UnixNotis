//! Supported runtime binary names declared by release archives

use std::collections::HashSet;
use std::path::{Component, Path};

use anyhow::{anyhow, Result};

// Release archives may install only the runtime targets produced by this workspace
const SUPPORTED_RELEASE_BINARIES: &[&str] = &[
    "unixnotis-daemon",
    "unixnotis-popups",
    "unixnotis-center",
    "unixnotis-css-validate",
    "noticenterctl",
];

pub fn validate_release_binary_names(names: Vec<String>) -> Result<Vec<String>> {
    let mut seen = HashSet::new();
    let mut binaries = Vec::with_capacity(names.len());

    for raw_name in names {
        let name = raw_name.trim();
        // One normal component keeps every source and destination below the release bin roots
        let mut components = Path::new(name).components();
        let is_single_component =
            matches!(components.next(), Some(Component::Normal(_))) && components.next().is_none();
        if !is_single_component {
            return Err(anyhow!(
                "release manifest contains an invalid binary path: {raw_name:?}"
            ));
        }
        if !SUPPORTED_RELEASE_BINARIES.contains(&name) {
            return Err(anyhow!(
                "release manifest contains unsupported binary name: {raw_name:?}"
            ));
        }

        if seen.insert(name.to_string()) {
            // First mention wins so release archive order stays deterministic
            binaries.push(name.to_string());
        }
    }

    if binaries.is_empty() {
        return Err(anyhow!(
            "release manifest does not list any supported binaries"
        ));
    }

    Ok(binaries)
}
