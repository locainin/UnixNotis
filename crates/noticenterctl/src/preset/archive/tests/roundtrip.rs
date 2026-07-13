use super::super::{read_bundle, write_bundle};
use super::support::TempDirGuard;
use crate::preset::config_root::{CollectedConfigFiles, PresetFileSource};
use crate::preset::manifest::{PresetManifest, PresetManifestFile};
use std::path::PathBuf;

#[test]
fn archive_round_trip_keeps_manifest_and_payload() {
    // Bundle reads should return the same file list that export wrote
    let root = TempDirGuard::new("roundtrip");
    let config_path = root.write("config.toml", "demo = true");
    let css_path = root.write("base.css", ".a { color: red; }");
    let bundle_path = root.path.join("demo.unixnotis");

    let collected = CollectedConfigFiles {
        files: vec![
            PresetFileSource {
                relative_path: PathBuf::from("base.css"),
                source_path: css_path,
                size: 18,
                mode: 0o644,
                source_contents: b".a { color: red; }".to_vec(),
                contents_override: None,
            },
            PresetFileSource {
                relative_path: PathBuf::from("config.toml"),
                source_path: config_path,
                size: 11,
                mode: 0o644,
                source_contents: b"demo = true".to_vec(),
                contents_override: None,
            },
        ],
        skipped_symlinks: Vec::new(),
        skipped_non_regular: Vec::new(),
    };
    let manifest = PresetManifest::new(
        "demo".to_string(),
        "2026-04-11T12:00:00Z".to_string(),
        "0.1.0".to_string(),
        vec![
            PresetManifestFile {
                path: "base.css".to_string(),
                size: 18,
            },
            PresetManifestFile {
                path: "config.toml".to_string(),
                size: 11,
            },
        ],
    );

    write_bundle(&bundle_path, &manifest, &collected).expect("write bundle");
    let bundle = read_bundle(&bundle_path).expect("read bundle");

    assert_eq!(bundle.manifest.bundle_name, "demo");
    assert_eq!(bundle.files.len(), 2);
}

#[test]
fn archive_round_trip_uses_overridden_file_bytes() {
    let root = TempDirGuard::new("override");
    let config_path = root.write("config.toml", "demo = true");
    let bundle_path = root.path.join("demo.unixnotis");

    let collected = CollectedConfigFiles {
        files: vec![PresetFileSource {
            relative_path: PathBuf::from("config.toml"),
            source_path: config_path,
            size: 12,
            mode: 0o644,
            source_contents: b"demo = true".to_vec(),
            contents_override: Some(b"demo = false\n".to_vec()),
        }],
        skipped_symlinks: Vec::new(),
        skipped_non_regular: Vec::new(),
    };
    let manifest = PresetManifest::new(
        "demo".to_string(),
        "2026-04-11T12:00:00Z".to_string(),
        "0.1.0".to_string(),
        vec![PresetManifestFile {
            path: "config.toml".to_string(),
            size: 13,
        }],
    );

    write_bundle(&bundle_path, &manifest, &collected).expect("write bundle");
    let bundle = read_bundle(&bundle_path).expect("read bundle");

    assert_eq!(bundle.files.len(), 1);
    assert_eq!(bundle.files[0].contents, b"demo = false\n");
}
