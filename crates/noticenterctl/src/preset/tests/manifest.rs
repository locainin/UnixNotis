use super::super::manifest::{PresetManifest, PresetManifestFile, PRESET_FORMAT_VERSION};

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

#[test]
fn manifest_flags_stay_false_when_bundle_has_only_config_files() {
    let manifest = PresetManifest::new(
        "minimal".to_string(),
        "2026-04-11T12:00:00Z".to_string(),
        "0.1.0".to_string(),
        vec![PresetManifestFile {
            path: "config.toml".to_string(),
            size: 10,
        }],
    );

    assert!(!manifest.has_assets);
    assert!(!manifest.has_scripts);
}
