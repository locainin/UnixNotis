use std::fs;
use std::os::unix::fs::{symlink, PermissionsExt};
use std::path::{Path, PathBuf};

use super::super::manifest::{entrypoint_target, inspect_installed_generation, BinaryHealth};
use super::super::transaction::commit_pending_release;
use super::install_release_generation;
use crate::paths::InstallPaths;
use crate::service_manager::ServiceManager;

fn paths(root: &Path) -> InstallPaths {
    InstallPaths {
        repo_root: root.join("repo"),
        bin_dir: root.join("home").join(".local").join("bin"),
        service: ServiceManager::systemd_user(
            root.join("home")
                .join(".config")
                .join("systemd")
                .join("user"),
        ),
    }
}

fn install_one_binary(label: &str) -> (PathBuf, InstallPaths, String, PathBuf) {
    let root = crate::test_support::fs::unique_temp_path(label);
    let source = root.join("source");
    fs::create_dir_all(&source).expect("create release source");
    let name = "unixnotis-daemon".to_string();
    write_test_binary(&source.join(&name), "healthy payload");
    let paths = paths(&root);
    let generation = install_release_generation(
        &paths,
        &source,
        std::slice::from_ref(&name),
        || Ok(()),
        || Ok(()),
    )
    .expect("install health fixture");
    commit_pending_release(&paths).expect("commit health fixture");
    let binary = paths
        .installed_releases_dir()
        .expect("releases directory")
        .join(&generation)
        .join("bin")
        .join(&name);
    (root, paths, name, binary)
}

fn health_for(paths: &InstallPaths, name: &str) -> BinaryHealth {
    inspect_installed_generation(paths, &[name.to_string()])
        .into_iter()
        .next()
        .expect("one binary health result")
        .1
}

#[test]
fn installed_generation_health_distinguishes_every_binary_failure_class() {
    let (root, paths, name, binary) = install_one_binary("release-health-classes");
    let entry = paths.bin_dir.join(&name);
    let expected_entry = entrypoint_target().join(&name);
    let original = fs::read(&binary).expect("read original release binary");

    assert!(matches!(
        health_for(&paths, &name),
        BinaryHealth::Healthy { .. }
    ));

    fs::remove_file(&entry).expect("remove managed entrypoint");
    assert_eq!(health_for(&paths, &name), BinaryHealth::Missing);
    symlink(&expected_entry, &entry).expect("restore managed entrypoint");

    fs::remove_file(&entry).expect("remove managed entrypoint");
    fs::write(&entry, "legacy file").expect("write wrong entrypoint type");
    assert_eq!(health_for(&paths, &name), BinaryHealth::WrongType);
    fs::remove_file(&entry).expect("remove wrong entrypoint type");
    symlink(Path::new("unmanaged-target"), &entry).expect("write wrong entrypoint link");
    assert_eq!(health_for(&paths, &name), BinaryHealth::WrongGeneration);
    fs::remove_file(&entry).expect("remove wrong entrypoint link");
    symlink(&expected_entry, &entry).expect("restore managed entrypoint");

    fs::set_permissions(&binary, fs::Permissions::from_mode(0o644))
        .expect("remove executable bits");
    assert_eq!(health_for(&paths, &name), BinaryHealth::NotExecutable);
    fs::set_permissions(&binary, fs::Permissions::from_mode(0o755))
        .expect("restore executable bits");

    fs::write(&binary, vec![b'x'; original.len()]).expect("write same-size changed binary");
    assert_eq!(health_for(&paths, &name), BinaryHealth::HashMismatch);
    fs::write(&binary, &original[..original.len() - 1]).expect("write truncated binary");
    assert_eq!(health_for(&paths, &name), BinaryHealth::WrongType);

    fs::remove_file(&binary).expect("remove release binary");
    assert_eq!(health_for(&paths, &name), BinaryHealth::BrokenLink);
    fs::create_dir(&binary).expect("create unsafe release binary object");
    assert!(matches!(health_for(&paths, &name), BinaryHealth::Unsafe(_)));

    fs::remove_dir_all(root).expect("remove release health fixture");
}

#[test]
fn missing_generation_classifies_missing_broken_and_legacy_entrypoints() {
    let root = crate::test_support::fs::unique_temp_path("release-health-no-current");
    let paths = paths(&root);
    fs::create_dir_all(&paths.bin_dir).expect("create entrypoint directory");
    symlink("missing-target", paths.bin_dir.join("broken")).expect("create broken entrypoint");
    fs::write(paths.bin_dir.join("legacy"), "legacy binary").expect("create legacy entrypoint");

    let health = inspect_installed_generation(
        &paths,
        &[
            "missing".to_string(),
            "broken".to_string(),
            "legacy".to_string(),
        ],
    );

    assert_eq!(health[0].1, BinaryHealth::Missing);
    assert_eq!(health[1].1, BinaryHealth::BrokenLink);
    assert_eq!(health[2].1, BinaryHealth::WrongGeneration);
    fs::remove_dir_all(root).expect("remove missing generation fixture");
}

#[test]
fn invalid_current_release_objects_never_count_as_an_installed_generation() {
    let regular_root = crate::test_support::fs::unique_temp_path("release-health-current-file");
    let regular_paths = paths(&regular_root);
    let current = regular_paths
        .installed_current_link()
        .expect("current link path");
    fs::create_dir_all(current.parent().expect("current parent")).expect("create install root");
    fs::write(&current, "not a link").expect("create wrong current object");
    assert!(matches!(
        health_for(&regular_paths, "unixnotis-daemon"),
        BinaryHealth::Unsafe(_)
    ));

    let foreign_root = crate::test_support::fs::unique_temp_path("release-health-foreign-link");
    let foreign_paths = paths(&foreign_root);
    let current = foreign_paths
        .installed_current_link()
        .expect("current link path");
    fs::create_dir_all(current.parent().expect("current parent")).expect("create install root");
    symlink(Path::new("foreign").join("generation"), &current)
        .expect("create foreign current link");
    assert_eq!(
        health_for(&foreign_paths, "unixnotis-daemon"),
        BinaryHealth::WrongGeneration
    );

    fs::remove_dir_all(regular_root).expect("remove current file fixture");
    fs::remove_dir_all(foreign_root).expect("remove foreign current fixture");
}

#[test]
fn entrypoint_lookup_errors_remain_unsafe_with_and_without_a_current_generation() {
    let missing_root = crate::test_support::fs::unique_temp_path("release-health-entry-error");
    let missing_paths = paths(&missing_root);
    fs::create_dir_all(missing_paths.bin_dir.parent().expect("entrypoint parent"))
        .expect("create entrypoint parent");
    fs::write(&missing_paths.bin_dir, "not a directory").expect("create invalid entrypoint root");
    assert!(matches!(
        health_for(&missing_paths, "unixnotis-daemon"),
        BinaryHealth::Unsafe(_)
    ));

    let (installed_root, installed_paths, name, _binary) =
        install_one_binary("release-health-installed-entry-error");
    fs::remove_file(installed_paths.bin_dir.join(&name)).expect("remove managed entrypoint");
    fs::remove_dir(&installed_paths.bin_dir).expect("remove entrypoint directory");
    fs::write(&installed_paths.bin_dir, "not a directory")
        .expect("create invalid installed entrypoint root");
    assert!(matches!(
        health_for(&installed_paths, &name),
        BinaryHealth::Unsafe(_)
    ));

    fs::remove_dir_all(missing_root).expect("remove missing entrypoint error fixture");
    fs::remove_dir_all(installed_root).expect("remove installed entrypoint error fixture");
}

#[test]
fn installed_generation_recomputes_manifest_build_identity() {
    let (root, paths, name, binary) = install_one_binary("release-health-build-identity");
    let manifest_path = binary
        .parent()
        .and_then(Path::parent)
        .expect("release generation directory")
        .join("manifest.json");
    let mut manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(&manifest_path).expect("read installed manifest"))
            .expect("parse installed manifest");
    manifest["build_id"] = serde_json::Value::String("forged-build-id".to_string());
    fs::write(
        &manifest_path,
        serde_json::to_vec_pretty(&manifest).expect("serialize changed manifest"),
    )
    .expect("write changed manifest");

    assert!(matches!(
        health_for(&paths, &name),
        BinaryHealth::Unsafe(detail) if detail.contains("build identity is inconsistent")
    ));
    fs::remove_dir_all(root).expect("remove build identity fixture");
}

#[test]
fn installed_generation_requires_the_current_directory_to_match_its_manifest_identity() {
    let (root, paths, name, binary) = install_one_binary("release-health-directory-identity");
    let generation_dir = binary
        .parent()
        .and_then(Path::parent)
        .expect("release generation directory");
    let renamed = generation_dir
        .parent()
        .expect("release generations parent")
        .join("renamed-generation");
    fs::rename(generation_dir, &renamed).expect("rename generation away from manifest identity");
    let current = paths
        .installed_current_link()
        .expect("current generation link");
    fs::remove_file(&current).expect("remove prior current link");
    symlink(Path::new("releases").join("renamed-generation"), &current)
        .expect("point current at renamed generation");

    assert_eq!(health_for(&paths, &name), BinaryHealth::WrongGeneration);
    fs::remove_dir_all(root).expect("remove directory identity fixture");
}

#[test]
fn installed_generation_rejects_unmanaged_manifest_binary_names() {
    let (root, paths, name, binary) = install_one_binary("release-health-managed-names");
    let manifest_path = binary
        .parent()
        .and_then(Path::parent)
        .expect("release generation directory")
        .join("manifest.json");
    let mut manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(&manifest_path).expect("read installed manifest"))
            .expect("parse installed manifest");
    let existing = manifest["binaries"][&name].clone();
    manifest["binaries"]["../outside"] = existing;
    fs::write(
        &manifest_path,
        serde_json::to_vec_pretty(&manifest).expect("serialize changed manifest"),
    )
    .expect("write changed manifest");

    assert!(matches!(
        health_for(&paths, &name),
        BinaryHealth::Unsafe(detail) if detail.contains("unmanaged binary names")
    ));
    fs::remove_dir_all(root).expect("remove managed-name fixture");
}

fn write_test_binary(path: &Path, contents: &str) {
    fs::write(path, contents).expect("write release test binary");
    fs::set_permissions(path, fs::Permissions::from_mode(0o755))
        .expect("make release test binary executable");
}
