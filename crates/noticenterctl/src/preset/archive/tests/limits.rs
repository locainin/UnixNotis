use super::super::budget::MAX_PRESET_COMPRESSED_BYTES;
use super::super::preflight::{MAX_PRESET_ARCHIVE_ENTRIES, MAX_PRESET_EXTENSION_METADATA_BYTES};
use super::super::read::{
    checked_payload_total, read_bundle, read_bundle_with_limits, MAX_PRESET_FILE_BYTES,
    MAX_PRESET_MANIFEST_BYTES, MAX_PRESET_PAYLOAD_FILES, MAX_PRESET_TOTAL_PAYLOAD_BYTES,
};
use super::support::{
    append_raw_tar_dir, append_raw_tar_file, append_raw_tar_header, write_raw_gzip_tar,
    TempDirGuard,
};
use crate::preset::manifest::{PresetManifest, PresetManifestFile};
use std::io::Write as _;
use std::path::Path;

#[test]
fn read_bundle_rejects_compressed_input_over_budget_before_decoding() {
    let root = TempDirGuard::new("compressed-input-budget");
    let bundle_path = root.path.join("demo.unixnotis");
    let bundle = std::fs::File::create(&bundle_path).expect("create sparse bundle");
    bundle
        .set_len(MAX_PRESET_COMPRESSED_BYTES + 1)
        .expect("size sparse bundle");

    let error = read_bundle(&bundle_path).expect_err("reject oversized compressed bundle");

    assert!(error.to_string().contains("compressed bytes"));
}

#[test]
fn read_bundle_accepts_compressed_input_at_the_exact_budget() {
    let root = TempDirGuard::new("compressed-input-exact-budget");
    let bundle_path = root.path.join("demo.unixnotis");
    write_raw_gzip_tar(&bundle_path, |_| {});
    let compressed_size = std::fs::metadata(&bundle_path)
        .expect("read bundle metadata")
        .len();

    let error = read_bundle_with_limits(
        &bundle_path,
        compressed_size,
        super::super::budget::MAX_PRESET_DECOMPRESSED_BYTES,
    )
    .expect_err("empty archive should still require a manifest");

    assert!(
        error.to_string().contains("missing manifest.toml"),
        "{error:#}"
    );
}

#[test]
fn read_bundle_uses_effective_pax_size_for_payload_limits() {
    let root = TempDirGuard::new("pax-size-override");
    let bundle_path = root.path.join("demo.unixnotis");
    let effective_size = MAX_PRESET_FILE_BYTES + 1;
    let pax = pax_record("size", &effective_size.to_string());

    write_raw_gzip_tar(&bundle_path, |encoder| {
        append_raw_tar_file(
            encoder,
            Path::new("manifest.toml"),
            b"not valid toml",
            0o644,
        );
        append_extension_entry(encoder, tar::EntryType::XHeader, &pax);
        // The raw header claims zero while the preceding PAX record overrides the effective size
        append_raw_tar_header(encoder, Path::new("payload/assets/bomb.bin"), 0, 0o644);
    });

    let error = read_bundle(&bundle_path).expect_err("PAX effective size must be bounded");

    assert!(error.to_string().contains("payload entry is too large"));
}

#[test]
fn read_bundle_accepts_a_bounded_pax_size_override() {
    let root = TempDirGuard::new("bounded-pax-size-override");
    let bundle_path = root.path.join("demo.unixnotis");
    let payload = vec![b'a'; 513];
    let manifest = PresetManifest::new(
        "demo".to_string(),
        "2026-04-11T12:00:00Z".to_string(),
        "0.1.0".to_string(),
        vec![PresetManifestFile {
            path: "assets/pax.bin".to_string(),
            size: u64::try_from(payload.len()).expect("payload length fits u64"),
        }],
    );
    let manifest_bytes = manifest.encode().expect("encode manifest").into_bytes();
    let mut pax = pax_record("path", "payload/assets/pax.bin");
    pax.extend(pax_record("size", &payload.len().to_string()));

    write_raw_gzip_tar(&bundle_path, |encoder| {
        append_raw_tar_file(encoder, Path::new("manifest.toml"), &manifest_bytes, 0o644);
        append_extension_entry(encoder, tar::EntryType::XHeader, &pax);
        append_raw_tar_header(encoder, Path::new("payload/assets/pax.bin"), 0, 0o644);
        encoder
            .write_all(&payload)
            .expect("write PAX-sized payload");
        let padding = (512 - payload.len() % 512) % 512;
        encoder
            .write_all(&vec![0_u8; padding])
            .expect("write PAX-sized payload padding");
    });

    let bundle = read_bundle(&bundle_path).expect("read bounded PAX size override");

    assert_eq!(bundle.files.len(), 1);
    assert_eq!(bundle.files[0].contents, payload);
}

#[test]
fn read_bundle_accepts_an_empty_gzip_before_requiring_a_manifest() {
    let root = TempDirGuard::new("empty-gzip");
    let bundle_path = root.path.join("demo.unixnotis");
    let output = std::fs::File::create(&bundle_path).expect("create empty gzip");
    flate2::write::GzEncoder::new(output, flate2::Compression::default())
        .finish()
        .expect("finish empty gzip");

    let error = read_bundle(&bundle_path).expect_err("empty archive should require a manifest");

    assert!(
        error.to_string().contains("missing manifest.toml"),
        "{error:#}"
    );
}

#[test]
fn read_bundle_rejects_a_tar_header_with_a_mismatched_checksum() {
    let root = TempDirGuard::new("header-checksum");
    let bundle_path = root.path.join("demo.unixnotis");
    write_raw_gzip_tar(&bundle_path, |encoder| {
        let mut header = tar::Header::new_gnu();
        header.set_path("manifest.toml").expect("set manifest path");
        header.set_mode(0o644);
        header.set_size(0);
        header.set_cksum();
        header.as_mut_bytes()[0] ^= 1;
        encoder
            .write_all(header.as_bytes())
            .expect("write header with mismatched checksum");
    });

    let error = read_bundle(&bundle_path).expect_err("reject mismatched tar checksum");

    assert!(
        error.to_string().contains("tar checksum mismatch"),
        "{error:#}"
    );
}

#[test]
fn read_bundle_bounds_hidden_pax_metadata_before_tar_yields_an_entry() {
    assert_hidden_extension_hits_decompressed_budget(
        "pax-metadata-budget",
        tar::EntryType::XHeader,
    );
}

#[test]
fn read_bundle_bounds_hidden_gnu_long_name_before_tar_yields_an_entry() {
    assert_hidden_extension_hits_decompressed_budget(
        "gnu-long-name-budget",
        tar::EntryType::GNULongName,
    );
}

#[test]
fn read_bundle_rejects_oversized_hidden_pax_metadata_before_tar_allocates_it() {
    assert_oversized_extension_metadata_is_rejected(
        "oversized-pax-metadata",
        tar::EntryType::XHeader,
    );
}

#[test]
fn read_bundle_rejects_oversized_hidden_gnu_long_name_before_tar_allocates_it() {
    assert_oversized_extension_metadata_is_rejected(
        "oversized-gnu-long-name",
        tar::EntryType::GNULongName,
    );
}

#[test]
fn read_bundle_allows_extension_metadata_at_the_exact_budget_through_preflight() {
    let root = TempDirGuard::new("extension-metadata-exact-budget");
    let bundle_path = root.path.join("demo.unixnotis");
    let extension = vec![
        b'A';
        usize::try_from(MAX_PRESET_EXTENSION_METADATA_BYTES)
            .expect("metadata limit fits usize")
    ];
    write_raw_gzip_tar(&bundle_path, |encoder| {
        append_extension_entry(encoder, tar::EntryType::GNULongName, &extension);
        append_raw_tar_file(encoder, Path::new("manifest.toml"), b"", 0o644);
    });

    let error = read_bundle(&bundle_path).expect_err("long path should fail later validation");

    assert!(
        !error
            .to_string()
            .contains("extension metadata is too large"),
        "{error:#}"
    );
}

fn pax_record(key: &str, value: &str) -> Vec<u8> {
    let body = format!("{key}={value}\n");
    let mut length = body.len() + 2;
    loop {
        let record = format!("{length} {body}");
        if record.len() == length {
            return record.into_bytes();
        }
        length = record.len();
    }
}

fn append_extension_entry(
    encoder: &mut flate2::write::GzEncoder<std::fs::File>,
    entry_type: tar::EntryType,
    contents: &[u8],
) {
    let mut header = tar::Header::new_gnu();
    header
        .set_path("ExtensionHeader")
        .expect("set extension path");
    header.set_entry_type(entry_type);
    header.set_mode(0o644);
    header.set_size(u64::try_from(contents.len()).expect("extension length fits u64"));
    header.set_cksum();
    encoder
        .write_all(header.as_bytes())
        .expect("write extension header");
    encoder
        .write_all(contents)
        .expect("write extension contents");
    let padding = (512 - contents.len() % 512) % 512;
    encoder
        .write_all(&vec![0_u8; padding])
        .expect("write extension padding");
}

fn assert_hidden_extension_hits_decompressed_budget(name: &str, entry_type: tar::EntryType) {
    const TEST_DECOMPRESSED_LIMIT: u64 = 32 * 1024;
    const EXTENSION_BYTES: usize = 64 * 1024;

    let root = TempDirGuard::new(name);
    let bundle_path = root.path.join("demo.unixnotis");
    let extension = vec![b'A'; EXTENSION_BYTES];
    write_raw_gzip_tar(&bundle_path, |encoder| {
        append_extension_entry(encoder, entry_type, &extension);
        // tar consumes the extension body internally before this ordinary member is yielded
        append_raw_tar_file(encoder, Path::new("manifest.toml"), b"", 0o644);
    });

    let error = read_bundle_with_limits(
        &bundle_path,
        MAX_PRESET_COMPRESSED_BYTES,
        TEST_DECOMPRESSED_LIMIT,
    )
    .expect_err("hidden extension metadata must be bounded");

    assert!(
        format!("{error:#}").contains("decompressed limit"),
        "{error:#}"
    );
}

fn assert_oversized_extension_metadata_is_rejected(name: &str, entry_type: tar::EntryType) {
    let root = TempDirGuard::new(name);
    let bundle_path = root.path.join("demo.unixnotis");
    let extension = vec![
        b'A';
        usize::try_from(MAX_PRESET_EXTENSION_METADATA_BYTES + 1)
            .expect("metadata limit fits usize")
    ];
    write_raw_gzip_tar(&bundle_path, |encoder| {
        append_extension_entry(encoder, entry_type, &extension);
        append_raw_tar_file(encoder, Path::new("manifest.toml"), b"", 0o644);
    });

    let error = read_bundle(&bundle_path).expect_err("reject oversized extension metadata");

    assert!(error
        .to_string()
        .contains("extension metadata is too large"));
}

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
fn read_bundle_rejects_sized_directory_before_decompressing_its_body() {
    let root = TempDirGuard::new("sized-directory-header");
    let bundle_path = root.path.join("demo.unixnotis");

    write_raw_gzip_tar(&bundle_path, |encoder| {
        let mut header = tar::Header::new_gnu();
        header
            .set_path("payload/assets")
            .expect("set directory path");
        header.set_entry_type(tar::EntryType::Directory);
        header.set_mode(0o755);
        header.set_size(MAX_PRESET_TOTAL_PAYLOAD_BYTES + 1);
        header.set_cksum();
        encoder
            .write_all(header.as_bytes())
            .expect("write sized directory header");
    });

    let error = read_bundle(&bundle_path).expect_err("sized directory must fail before body read");

    assert!(error
        .to_string()
        .contains("directory entry has a nonzero size"));
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

    assert!(
        error.to_string().contains("too many archive entries"),
        "{error:#}"
    );
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

    assert!(
        error.to_string().contains("missing manifest.toml"),
        "{error:#}"
    );
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
