//! Archive module root for `.unixnotis` bundles
//!
//! Keeps archive reads, writes, mode checks, and tests grouped under one tree
//! so the preset layer has one clear module boundary for bundle I/O

mod modes;
mod read;
#[cfg(test)]
mod tests;
mod write;

use std::path::PathBuf;

pub(super) use self::read::{
    read_bundle, MAX_PRESET_FILE_BYTES, MAX_PRESET_PAYLOAD_FILES, MAX_PRESET_TOTAL_PAYLOAD_BYTES,
};
pub(super) use self::write::write_bundle;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct BundleFile {
    // Relative path inside the UnixNotis config root
    pub(super) relative_path: PathBuf,
    // Full file bytes captured from the bundle
    pub(super) contents: Vec<u8>,
    // Stored mode is restored on import so scripts keep execute bits
    pub(super) mode: u32,
}

#[derive(Debug)]
pub(super) struct BundleArchive {
    // Manifest is loaded first so inspect and import can trust one source of truth
    pub(super) manifest: super::manifest::PresetManifest,
    // Payload files are kept separate from manifest metadata for simpler validation
    pub(super) files: Vec<BundleFile>,
}
