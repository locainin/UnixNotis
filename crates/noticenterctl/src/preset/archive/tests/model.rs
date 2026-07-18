use std::path::PathBuf;

use super::{BundleArchive, BundleFile};
use crate::preset::manifest::PresetManifest;

#[test]
fn archive_model_keeps_manifest_metadata_separate_from_file_bytes() {
    let file = BundleFile {
        relative_path: PathBuf::from("scripts/status"),
        contents: b"#!/bin/sh\n".to_vec(),
        mode: 0o755,
    };
    let manifest = PresetManifest::new(
        "portable".to_string(),
        "2026-01-01T00:00:00Z".to_string(),
        "1.1.5".to_string(),
        Vec::new(),
    );
    let archive = BundleArchive {
        manifest,
        files: vec![file],
    };

    assert_eq!(archive.manifest.bundle_name, "portable");
    assert_eq!(
        archive.files[0].relative_path,
        PathBuf::from("scripts/status")
    );
    assert_eq!(archive.files[0].mode, 0o755);
}
