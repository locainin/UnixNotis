//! Manifest types for shareable UnixNotis preset bundles
//!
//! The manifest is the stable metadata contract inside the bundle
//! Archive layout can evolve later while this stays the simple source of truth

use serde::{Deserialize, Serialize};

pub(super) const PRESET_FORMAT_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(super) struct PresetManifest {
    // Bumped only for breaking archive or metadata changes
    pub(super) format_version: u32,
    // Derived from the bundle file name and shown by inspect
    pub(super) bundle_name: String,
    // RFC3339 export timestamp for debugging and audit output
    pub(super) exported_at: String,
    // noticenterctl version that wrote the bundle
    pub(super) tool_version: String,
    // Fast summary flag used by inspect output
    pub(super) has_assets: bool,
    // Fast summary flag used by inspect output
    pub(super) has_scripts: bool,
    // Exact file list used to validate archive payload on read
    pub(super) files: Vec<PresetManifestFile>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(super) struct PresetManifestFile {
    // Slash-separated relative path inside the preset bundle
    pub(super) path: String,
    // Stored file size used to detect corrupt or mismatched payload entries
    pub(super) size: u64,
}

impl PresetManifest {
    pub(super) fn new(
        bundle_name: String,
        exported_at: String,
        tool_version: String,
        files: Vec<PresetManifestFile>,
    ) -> Self {
        // Flags are stored in the manifest so inspect stays cheap and simple
        let has_assets = files.iter().any(|file| file.path.starts_with("assets/"));
        let has_scripts = files.iter().any(|file| file.path.starts_with("scripts/"));
        Self {
            format_version: PRESET_FORMAT_VERSION,
            bundle_name,
            exported_at,
            tool_version,
            has_assets,
            has_scripts,
            files,
        }
    }

    pub(super) fn encode(&self) -> anyhow::Result<String> {
        // TOML keeps the archive easy to inspect by hand when debugging
        Ok(toml::to_string_pretty(self)?)
    }

    pub(super) fn decode(contents: &str) -> anyhow::Result<Self> {
        // Version checks happen after parse so decode can stay focused on shape
        Ok(toml::from_str(contents)?)
    }
}

#[cfg(test)]
mod tests {
    use super::{PresetManifest, PresetManifestFile, PRESET_FORMAT_VERSION};

    #[test]
    fn manifest_round_trip_preserves_file_flags() {
        // Asset and script flags should survive encode and decode intact
        let manifest = PresetManifest::new(
            "anime".to_string(),
            "2026-04-11T12:00:00Z".to_string(),
            "0.1.0".to_string(),
            vec![
                PresetManifestFile {
                    path: "config.toml".to_string(),
                    size: 10,
                },
                PresetManifestFile {
                    path: "assets/bg.png".to_string(),
                    size: 20,
                },
                PresetManifestFile {
                    path: "scripts/fetch.sh".to_string(),
                    size: 30,
                },
            ],
        );

        let encoded = manifest.encode().expect("encode manifest");
        let decoded = PresetManifest::decode(&encoded).expect("decode manifest");

        assert_eq!(decoded.format_version, PRESET_FORMAT_VERSION);
        assert!(decoded.has_assets);
        assert!(decoded.has_scripts);
        assert_eq!(decoded.files.len(), 3);
    }
}
