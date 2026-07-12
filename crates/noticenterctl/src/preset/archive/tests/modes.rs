use super::super::read_bundle;
use super::support::{append_raw_tar_file, write_raw_gzip_tar, TempDirGuard};
use crate::preset::manifest::{PresetManifest, PresetManifestFile};
use std::path::Path;

#[test]
fn read_bundle_rejects_special_permission_bits() {
    let root = TempDirGuard::new("special-mode");
    let bundle_path = root.path.join("demo.unixnotis");
    let manifest = PresetManifest::new(
        "demo".to_string(),
        "2026-04-11T12:00:00Z".to_string(),
        "0.1.0".to_string(),
        vec![PresetManifestFile {
            path: "config.toml".to_string(),
            size: 12,
        }],
    );
    let manifest_bytes = manifest.encode().expect("encode manifest").into_bytes();

    write_raw_gzip_tar(&bundle_path, |encoder| {
        append_raw_tar_file(encoder, Path::new("manifest.toml"), &manifest_bytes, 0o644);
        append_raw_tar_file(
            encoder,
            Path::new("payload/config.toml"),
            b"demo = true\n",
            0o4755,
        );
    });

    let error = read_bundle(&bundle_path).expect_err("reject special mode bits");

    assert!(error.to_string().contains("special permission bits"));
}
