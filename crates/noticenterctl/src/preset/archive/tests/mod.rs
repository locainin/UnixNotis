use super::{read_bundle, write_bundle};
use crate::preset::config_root::CollectedConfigFiles;
use crate::preset::config_root::PresetFileSource;
use crate::preset::manifest::{PresetManifest, PresetManifestFile};
use flate2::write::GzEncoder;
use flate2::Compression;
use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};
use tar::{Builder, Header};

static TEST_TEMP_COUNTER: AtomicUsize = AtomicUsize::new(0);

struct TempDirGuard {
    path: PathBuf,
}

impl TempDirGuard {
    fn new(name: &str) -> Self {
        // Unique temp roots keep tests isolated even when cargo runs them in parallel
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock moved backwards")
            .as_nanos();
        let serial = TEST_TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "unixnotis-preset-archive-{}-{}-{}",
            name, stamp, serial
        ));
        fs::create_dir_all(&path).expect("create temp dir");
        Self { path }
    }

    fn write(&self, relative_path: &str, contents: &str) -> PathBuf {
        // Test helpers build small fake config trees without touching the real config root
        let path = self.path.join(relative_path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create parent dirs");
        }
        fs::write(&path, contents).expect("write file");
        path
    }
}

impl Drop for TempDirGuard {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

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
                contents_override: None,
            },
            PresetFileSource {
                relative_path: PathBuf::from("config.toml"),
                source_path: config_path,
                size: 11,
                mode: 0o644,
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

#[test]
fn read_bundle_rejects_special_permission_bits() {
    let root = TempDirGuard::new("special-mode");
    let bundle_path = root.path.join("demo.unixnotis");
    let output = fs::File::create(&bundle_path).expect("create bundle");
    let encoder = GzEncoder::new(output, Compression::default());
    let mut builder = Builder::new(encoder);

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
    let mut manifest_header = Header::new_gnu();
    manifest_header.set_mode(0o644);
    manifest_header.set_size(manifest_bytes.len() as u64);
    manifest_header.set_cksum();
    builder
        .append_data(
            &mut manifest_header,
            Path::new("manifest.toml"),
            Cursor::new(manifest_bytes),
        )
        .expect("append manifest");

    let payload = b"demo = true\n";
    let mut payload_header = Header::new_gnu();
    payload_header.set_mode(0o4755);
    payload_header.set_size(payload.len() as u64);
    payload_header.set_cksum();
    builder
        .append_data(
            &mut payload_header,
            Path::new("payload/config.toml"),
            Cursor::new(payload),
        )
        .expect("append payload");
    builder.finish().expect("finish archive");
    let encoder = builder.into_inner().expect("take encoder");
    let file = encoder.finish().expect("finish gzip");
    file.sync_all().expect("sync bundle");

    let error = read_bundle(&bundle_path).expect_err("reject special mode bits");
    assert!(error.to_string().contains("special permission bits"));
}
