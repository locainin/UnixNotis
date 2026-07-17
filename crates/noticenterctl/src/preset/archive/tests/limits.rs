use super::super::read::{
    checked_payload_total, read_bundle, MAX_PRESET_ARCHIVE_ENTRIES, MAX_PRESET_FILE_BYTES,
    MAX_PRESET_MANIFEST_BYTES, MAX_PRESET_PAYLOAD_FILES, MAX_PRESET_TOTAL_PAYLOAD_BYTES,
};
use super::support::{
    append_raw_tar_dir, append_raw_tar_file, append_raw_tar_header, write_raw_gzip_tar,
    TempDirGuard,
};
use crate::preset::manifest::{PresetManifest, PresetManifestFile};
use std::path::Path;

#[test]
fn read_bundle_rejects_oversized_payload_header_before_reading_body() {
    let root = TempDirGuard::new("oversized-payload-header");
    let bundle_path = root.path.join("demo.unixnotis");
    let manifest = PresetManifest::new(
        "demo".to_string(),
        "2026-04-11T12:00:00Z".to_string(),
        "0.1.0".to_string(),
        vec![PresetManifestFile {
            path: "assets/bomb.bin".to_string(),
            size: MAX_PRESET_TOTAL_PAYLOAD_BYTES + 1,
        }],
    );
    let manifest_bytes = manifest.encode().expect("encode manifest").into_bytes();

    write_raw_gzip_tar(&bundle_path, |encoder| {
        append_raw_tar_file(encoder, Path::new("manifest.toml"), &manifest_bytes, 0o644);
        append_raw_tar_header(
            encoder,
            Path::new("payload/assets/bomb.bin"),
            MAX_PRESET_TOTAL_PAYLOAD_BYTES + 1,
            0o644,
        );
    });

    let error =
        read_bundle(&bundle_path).expect_err("oversized payload must fail before body read");

    assert!(error.to_string().contains("payload entry is too large"));
}

#[test]
fn read_bundle_rejects_oversized_manifest_header_before_reading_body() {
    let root = TempDirGuard::new("oversized-manifest-header");
    let bundle_path = root.path.join("demo.unixnotis");

    write_raw_gzip_tar(&bundle_path, |encoder| {
        append_raw_tar_header(
            encoder,
            Path::new("manifest.toml"),
            MAX_PRESET_MANIFEST_BYTES + 1,
            0o644,
        );
    });

    let error =
        read_bundle(&bundle_path).expect_err("oversized manifest must fail before body read");

    assert!(error.to_string().contains("manifest entry is too large"));
}

#[test]
fn read_bundle_rejects_archive_entry_count_over_budget() {
    let root = TempDirGuard::new("archive-entry-over-budget");
    let bundle_path = root.path.join("demo.unixnotis");

    write_raw_gzip_tar(&bundle_path, |encoder| {
        for index in 0..=MAX_PRESET_ARCHIVE_ENTRIES {
            append_raw_tar_dir(encoder, Path::new(&format!("ignored-{index}")));
        }
    });

    let error = read_bundle(&bundle_path).expect_err("archive entry count must be bounded");

    assert!(error.to_string().contains("too many archive entries"));
}

#[test]
fn read_bundle_allows_archive_entry_count_at_exact_budget() {
    let root = TempDirGuard::new("archive-entry-exact-budget");
    let bundle_path = root.path.join("demo.unixnotis");

    write_raw_gzip_tar(&bundle_path, |encoder| {
        for index in 0..MAX_PRESET_ARCHIVE_ENTRIES {
            append_raw_tar_dir(encoder, Path::new(&format!("ignored-{index}")));
        }
    });

    let error = read_bundle(&bundle_path).expect_err("manifest should still be required");

    assert!(error.to_string().contains("missing manifest.toml"));
}

#[test]
fn read_bundle_rejects_payload_file_count_over_budget() {
    let root = TempDirGuard::new("payload-count-over-budget");
    let bundle_path = root.path.join("demo.unixnotis");
    let files = zero_sized_manifest_files(MAX_PRESET_PAYLOAD_FILES);
    let manifest = manifest_with_files(files);
    let manifest_bytes = manifest.encode().expect("encode manifest").into_bytes();

    write_raw_gzip_tar(&bundle_path, |encoder| {
        append_raw_tar_file(encoder, Path::new("manifest.toml"), &manifest_bytes, 0o644);
        for index in 0..=MAX_PRESET_PAYLOAD_FILES {
            append_raw_tar_file(
                encoder,
                Path::new(&format!("payload/assets/{index}.svg")),
                b"",
                0o644,
            );
        }
    });

    let error = read_bundle(&bundle_path).expect_err("payload count must be bounded");

    assert!(error.to_string().contains("too many payload files"));
}

#[test]
fn read_bundle_allows_payload_file_count_at_exact_budget() {
    let root = TempDirGuard::new("payload-count-exact-budget");
    let bundle_path = root.path.join("demo.unixnotis");
    let files = zero_sized_manifest_files(MAX_PRESET_PAYLOAD_FILES);
    let manifest = manifest_with_files(files);
    let manifest_bytes = manifest.encode().expect("encode manifest").into_bytes();

    write_raw_gzip_tar(&bundle_path, |encoder| {
        append_raw_tar_file(encoder, Path::new("manifest.toml"), &manifest_bytes, 0o644);
        for index in 0..MAX_PRESET_PAYLOAD_FILES {
            append_raw_tar_file(
                encoder,
                Path::new(&format!("payload/assets/{index}.svg")),
                b"",
                0o644,
            );
        }
    });

    let bundle = read_bundle(&bundle_path).expect("exact payload count should be allowed");

    assert_eq!(bundle.files.len(), MAX_PRESET_PAYLOAD_FILES);
}

#[test]
fn read_bundle_rejects_manifest_that_claims_too_many_payload_files() {
    let root = TempDirGuard::new("manifest-too-many-files");
    let bundle_path = root.path.join("demo.unixnotis");
    let manifest = manifest_with_files(zero_sized_manifest_files(MAX_PRESET_PAYLOAD_FILES + 1));
    let manifest_bytes = manifest.encode().expect("encode manifest").into_bytes();

    write_raw_gzip_tar(&bundle_path, |encoder| {
        append_raw_tar_file(encoder, Path::new("manifest.toml"), &manifest_bytes, 0o644);
    });

    let error = read_bundle(&bundle_path).expect_err("manifest file count must be bounded");

    assert!(error.to_string().contains("too many payload files"));
}

#[test]
fn read_bundle_allows_manifest_file_count_at_exact_budget() {
    let root = TempDirGuard::new("manifest-file-count-exact-budget");
    let bundle_path = root.path.join("demo.unixnotis");
    let manifest = manifest_with_files(zero_sized_manifest_files(MAX_PRESET_PAYLOAD_FILES));
    let manifest_bytes = manifest.encode().expect("encode manifest").into_bytes();

    write_raw_gzip_tar(&bundle_path, |encoder| {
        append_raw_tar_file(encoder, Path::new("manifest.toml"), &manifest_bytes, 0o644);
    });

    let error = read_bundle(&bundle_path).expect_err("payload mismatch should still be checked");

    assert!(error.to_string().contains("file list does not match"));
}

#[test]
fn read_bundle_rejects_manifest_total_payload_budget_overflow() {
    let root = TempDirGuard::new("manifest-total-budget");
    let bundle_path = root.path.join("demo.unixnotis");
    let manifest = manifest_with_files(vec![
        PresetManifestFile {
            path: "assets/a.bin".to_string(),
            size: MAX_PRESET_FILE_BYTES,
        },
        PresetManifestFile {
            path: "assets/b.bin".to_string(),
            size: MAX_PRESET_FILE_BYTES,
        },
        PresetManifestFile {
            path: "assets/c.bin".to_string(),
            size: MAX_PRESET_FILE_BYTES,
        },
        PresetManifestFile {
            path: "assets/d.bin".to_string(),
            size: MAX_PRESET_FILE_BYTES,
        },
        PresetManifestFile {
            path: "assets/e.bin".to_string(),
            size: MAX_PRESET_FILE_BYTES,
        },
    ]);
    let manifest_bytes = manifest.encode().expect("encode manifest").into_bytes();

    write_raw_gzip_tar(&bundle_path, |encoder| {
        append_raw_tar_file(encoder, Path::new("manifest.toml"), &manifest_bytes, 0o644);
    });

    let error = read_bundle(&bundle_path).expect_err("manifest total payload must be bounded");

    assert!(error.to_string().contains("manifest payload is too large"));
}

#[test]
fn read_bundle_allows_manifest_total_payload_at_exact_budget() {
    let root = TempDirGuard::new("manifest-total-exact-budget");
    let bundle_path = root.path.join("demo.unixnotis");
    let manifest = manifest_with_files(vec![
        PresetManifestFile {
            path: "assets/a.bin".to_string(),
            size: MAX_PRESET_FILE_BYTES,
        },
        PresetManifestFile {
            path: "assets/b.bin".to_string(),
            size: MAX_PRESET_FILE_BYTES,
        },
        PresetManifestFile {
            path: "assets/c.bin".to_string(),
            size: MAX_PRESET_FILE_BYTES,
        },
        PresetManifestFile {
            path: "assets/d.bin".to_string(),
            size: MAX_PRESET_FILE_BYTES,
        },
    ]);
    let manifest_bytes = manifest.encode().expect("encode manifest").into_bytes();

    write_raw_gzip_tar(&bundle_path, |encoder| {
        append_raw_tar_file(encoder, Path::new("manifest.toml"), &manifest_bytes, 0o644);
    });

    let error = read_bundle(&bundle_path).expect_err("payload mismatch should still be checked");

    assert!(error.to_string().contains("file list does not match"));
}

#[test]
fn read_bundle_rejects_duplicate_manifest_entries() {
    let root = TempDirGuard::new("duplicate-manifest");
    let bundle_path = root.path.join("demo.unixnotis");
    let manifest = manifest_with_files(Vec::new());
    let manifest_bytes = manifest.encode().expect("encode manifest").into_bytes();

    write_raw_gzip_tar(&bundle_path, |encoder| {
        append_raw_tar_file(encoder, Path::new("manifest.toml"), &manifest_bytes, 0o644);
        append_raw_tar_file(encoder, Path::new("manifest.toml"), &manifest_bytes, 0o644);
    });

    let error = read_bundle(&bundle_path).expect_err("duplicate manifest must fail");

    assert!(error.to_string().contains("duplicate manifest"));
}

#[test]
fn read_bundle_rejects_duplicate_manifest_file_paths() {
    let root = TempDirGuard::new("duplicate-manifest-file-path");
    let bundle_path = root.path.join("demo.unixnotis");
    let manifest = manifest_with_files(vec![
        PresetManifestFile {
            path: "config.toml".to_string(),
            size: 0,
        },
        PresetManifestFile {
            path: "config.toml".to_string(),
            size: 0,
        },
    ]);
    let manifest_bytes = manifest.encode().expect("encode manifest").into_bytes();

    write_raw_gzip_tar(&bundle_path, |encoder| {
        append_raw_tar_file(encoder, Path::new("manifest.toml"), &manifest_bytes, 0o644);
        append_raw_tar_file(encoder, Path::new("payload/config.toml"), b"", 0o644);
    });

    let error = read_bundle(&bundle_path).expect_err("duplicate manifest paths must fail");

    assert!(error.to_string().contains("duplicate file path"));
}

#[test]
fn read_bundle_rejects_spoofed_script_summary() {
    let root = TempDirGuard::new("spoofed-script-summary");
    let bundle_path = root.path.join("demo.unixnotis");
    let mut manifest = manifest_with_files(vec![PresetManifestFile {
        path: "scripts/run".to_string(),
        size: 0,
    }]);
    manifest.has_scripts = false;
    let manifest_bytes = manifest.encode().expect("encode manifest").into_bytes();

    write_raw_gzip_tar(&bundle_path, |encoder| {
        append_raw_tar_file(encoder, Path::new("manifest.toml"), &manifest_bytes, 0o644);
        append_raw_tar_file(encoder, Path::new("payload/scripts/run"), b"", 0o755);
    });

    let error = read_bundle(&bundle_path).expect_err("spoofed script flag must fail");

    assert!(error.to_string().contains("summary does not match"));
}

#[test]
fn checked_payload_total_allows_exact_total_payload_budget() {
    let total = checked_payload_total(MAX_PRESET_TOTAL_PAYLOAD_BYTES - 1, 1)
        .expect("exact total budget should be allowed");

    assert_eq!(total, MAX_PRESET_TOTAL_PAYLOAD_BYTES);
}

#[test]
fn checked_payload_total_rejects_total_payload_budget_overflow() {
    let error = checked_payload_total(MAX_PRESET_TOTAL_PAYLOAD_BYTES, 1)
        .expect_err("payload total over budget should fail");

    assert!(error.to_string().contains("payload is too large"));
}

fn zero_sized_manifest_files(count: usize) -> Vec<PresetManifestFile> {
    (0..count)
        .map(|index| PresetManifestFile {
            path: format!("assets/{index}.svg"),
            size: 0,
        })
        .collect()
}

fn manifest_with_files(files: Vec<PresetManifestFile>) -> PresetManifest {
    PresetManifest::new(
        "demo".to_string(),
        "2026-04-11T12:00:00Z".to_string(),
        "0.1.0".to_string(),
        files,
    )
}
