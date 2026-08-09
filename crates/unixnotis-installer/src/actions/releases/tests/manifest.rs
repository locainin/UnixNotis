use super::super::manifest::{
    build_manifest, verify_release_directory, BinaryHealth, HASH_BUFFER_BYTES,
    INSTALLED_MANIFEST_FILE, MAX_INSTALLED_MANIFEST_BYTES,
};
use std::os::unix::fs::PermissionsExt;

#[test]
fn manifest_records_a_digest_for_every_declared_binary() {
    let root = crate::test_support::fs::unique_temp_path("release-manifest-digests");
    std::fs::create_dir_all(root.join("bin")).expect("create release test root");
    let source = root.join("source");
    write_test_binary(&source, "binary payload");

    let manifest = build_manifest(&[("unixnotis-daemon".to_string(), source)])
        .expect("build release manifest");

    let binary = manifest
        .binaries
        .get("unixnotis-daemon")
        .expect("daemon manifest entry");
    assert_eq!(binary.size, 14);
    assert_eq!(binary.sha256.len(), 64);
    std::fs::remove_dir_all(root).expect("remove release manifest fixture");
}

#[test]
fn release_verification_rejects_a_changed_binary_digest() {
    let root = crate::test_support::fs::unique_temp_path("release-manifest-mismatch");
    let source = root.join("source");
    let release = root.join("release");
    std::fs::create_dir_all(release.join("bin")).expect("create release fixture");
    write_test_binary(&source, "original");
    let manifest = build_manifest(&[("unixnotis-daemon".to_string(), source)])
        .expect("build release manifest");
    write_test_binary(&release.join("bin").join("unixnotis-daemon"), "changed!");
    std::fs::write(
        release.join(INSTALLED_MANIFEST_FILE),
        serde_json::to_vec_pretty(&manifest).expect("serialize manifest"),
    )
    .expect("write manifest");

    let error = verify_release_directory(&release, &manifest)
        .expect_err("changed binary must fail verification");

    assert!(error.to_string().contains("digest mismatch"));
    std::fs::remove_dir_all(root).expect("remove release mismatch fixture");
}

#[test]
fn release_security_limits_keep_their_declared_byte_domains() {
    assert_eq!(MAX_INSTALLED_MANIFEST_BYTES, 262_144);
    assert_eq!(HASH_BUFFER_BYTES, 65_536);
}

#[test]
fn binary_health_labels_cover_every_installed_state() {
    let states = [
        (BinaryHealth::Missing, "missing"),
        (
            BinaryHealth::Healthy {
                generation: "generation".to_string(),
                package_version: "1.2.0".to_string(),
                digest: "digest".to_string(),
            },
            "healthy",
        ),
        (BinaryHealth::WrongType, "wrong type"),
        (BinaryHealth::NotExecutable, "not executable"),
        (BinaryHealth::BrokenLink, "broken link"),
        (BinaryHealth::WrongGeneration, "wrong generation"),
        (BinaryHealth::HashMismatch, "hash mismatch"),
        (BinaryHealth::Unsafe("detail".to_string()), "unsafe"),
    ];

    for (state, expected) in states {
        assert_eq!(state.label(), expected);
    }
    assert!(!matches!(
        BinaryHealth::Missing,
        BinaryHealth::Healthy { .. }
    ));
    assert!(!matches!(
        BinaryHealth::WrongType,
        BinaryHealth::Healthy { .. }
    ));
}

#[test]
fn manifest_construction_rejects_a_non_executable_source() {
    let root = crate::test_support::fs::unique_temp_path("release-manifest-source-mode");
    let source = root.join("unixnotis-daemon");
    std::fs::create_dir_all(&root).expect("create source mode fixture");
    std::fs::write(&source, "binary payload").expect("write non-executable source");
    std::fs::set_permissions(&source, std::fs::Permissions::from_mode(0o644))
        .expect("set non-executable mode");

    let error = build_manifest(&[("unixnotis-daemon".to_string(), source)])
        .expect_err("non-executable sources must not enter a release manifest");

    assert!(error.to_string().contains("not executable"));
    std::fs::remove_dir_all(root).expect("remove source mode fixture");
}

#[test]
fn release_verification_rejects_a_non_executable_installed_binary() {
    let root = crate::test_support::fs::unique_temp_path("release-manifest-installed-mode");
    let source = root.join("source");
    let release = root.join("release");
    std::fs::create_dir_all(release.join("bin")).expect("create release fixture");
    write_test_binary(&source, "binary payload");
    let manifest = build_manifest(&[("unixnotis-daemon".to_string(), source)])
        .expect("build release manifest");
    let installed = release.join("bin").join("unixnotis-daemon");
    std::fs::write(&installed, "binary payload").expect("write installed binary");
    std::fs::set_permissions(&installed, std::fs::Permissions::from_mode(0o644))
        .expect("remove installed executable bits");
    std::fs::write(
        release.join(INSTALLED_MANIFEST_FILE),
        serde_json::to_vec_pretty(&manifest).expect("serialize manifest"),
    )
    .expect("write manifest");

    let error = verify_release_directory(&release, &manifest)
        .expect_err("non-executable installed binaries must fail verification");

    assert!(error.to_string().contains("not executable"));
    std::fs::remove_dir_all(root).expect("remove installed mode fixture");
}

fn write_test_binary(path: &std::path::Path, contents: &str) {
    std::fs::write(path, contents).expect("write release test binary");
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755))
        .expect("make release test binary executable");
}
