//! In-memory archive records shared by preset readers and writers

use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::preset) struct BundleFile {
    // Relative path inside the UnixNotis config root
    pub(in crate::preset) relative_path: PathBuf,
    // Full file bytes captured from the bundle
    pub(in crate::preset) contents: Vec<u8>,
    // Stored mode is restored on import so scripts keep execute bits
    pub(in crate::preset) mode: u32,
}

#[derive(Debug)]
pub(in crate::preset) struct BundleArchive {
    // Manifest is loaded first so inspect and import can trust one source of truth
    pub(in crate::preset) manifest: super::super::manifest::PresetManifest,
    // Payload files are kept separate from manifest metadata for simpler validation
    pub(in crate::preset) files: Vec<BundleFile>,
}

#[cfg(test)]
#[path = "tests/model.rs"]
mod tests;
