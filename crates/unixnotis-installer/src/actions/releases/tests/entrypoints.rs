use std::fs;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};

use super::super::entrypoints::rollback_entrypoint_changes;
use super::super::transaction::{PendingRelease, PENDING_RELEASE_SCHEMA_VERSION};
use crate::paths::InstallPaths;
use crate::service_manager::ServiceManager;

const BINARY: &str = "unixnotis-daemon";

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

fn pending() -> PendingRelease {
    PendingRelease {
        schema_version: PENDING_RELEASE_SCHEMA_VERSION,
        generation: "test-generation".to_string(),
        new_current: PathBuf::from("releases/test-generation"),
        previous_current: None,
        legacy_entrypoints: vec![BINARY.to_string()],
        created_entrypoints: Vec::new(),
    }
}

#[test]
fn rollback_rejects_a_symbolic_link_instead_of_a_regular_backup() {
    let root = crate::test_support::fs::unique_temp_path("release-backup-link");
    let paths = paths(&root);
    let pending = pending();
    let backup = rollback_backup(&paths, &pending);
    fs::create_dir_all(backup.parent().expect("backup parent")).expect("create backup parent");
    symlink("unexpected", &backup).expect("create invalid backup link");

    let error = rollback_entrypoint_changes(&paths, &pending)
        .expect_err("a symbolic-link backup must fail closed");

    assert!(error.to_string().contains("unexpected symbolic link"));
    fs::remove_dir_all(root).expect("remove backup link fixture");
}

#[test]
fn rollback_rejects_a_directory_instead_of_a_regular_backup() {
    let root = crate::test_support::fs::unique_temp_path("release-backup-directory");
    let paths = paths(&root);
    let pending = pending();
    fs::create_dir_all(&paths.bin_dir).expect("create entrypoint directory");
    fs::write(paths.bin_dir.join(BINARY), "legacy").expect("write live legacy binary");
    let backup = rollback_backup(&paths, &pending);
    fs::create_dir_all(&backup).expect("create invalid backup directory");

    let error = rollback_entrypoint_changes(&paths, &pending)
        .expect_err("a directory backup must fail closed");

    assert!(error.to_string().contains("not a regular file"));
    fs::remove_dir_all(root).expect("remove backup directory fixture");
}

#[test]
fn rollback_propagates_backup_lookup_errors_instead_of_treating_them_as_missing() {
    let root = crate::test_support::fs::unique_temp_path("release-backup-lookup-error");
    let paths = paths(&root);
    let pending = pending();
    fs::create_dir_all(&paths.bin_dir).expect("create entrypoint directory");
    fs::write(paths.bin_dir.join(BINARY), "legacy").expect("write live legacy binary");
    let rollback_generation = paths
        .installed_rollback_root()
        .expect("rollback root")
        .join(&pending.generation);
    fs::create_dir_all(rollback_generation.parent().expect("rollback parent"))
        .expect("create rollback parent");
    fs::write(&rollback_generation, "not a directory").expect("write invalid rollback object");

    let error = rollback_entrypoint_changes(&paths, &pending)
        .expect_err("backup lookup errors must remain errors");

    assert!(error.to_string().contains("inspect"));
    fs::remove_dir_all(root).expect("remove backup lookup fixture");
}

fn rollback_backup(paths: &InstallPaths, pending: &PendingRelease) -> PathBuf {
    paths
        .installed_rollback_root()
        .expect("rollback root")
        .join(&pending.generation)
        .join("bin")
        .join(BINARY)
}
