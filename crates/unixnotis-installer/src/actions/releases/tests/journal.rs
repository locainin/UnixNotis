use std::fs;
use std::path::{Path, PathBuf};

use super::super::entrypoints::plan_entrypoint_changes;
use super::super::transaction::{
    is_managed_current_target, pending_release_has_runtime_rollback, read_pending,
    validate_pending_targets, write_pending, PendingRelease, MAX_PENDING_MANIFEST_BYTES,
    PENDING_RELEASE_SCHEMA_VERSION,
};
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

fn pending(new_current: &str, previous_current: Option<&str>) -> PendingRelease {
    PendingRelease {
        schema_version: PENDING_RELEASE_SCHEMA_VERSION,
        generation: Path::new(new_current)
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("missing")
            .to_string(),
        new_current: PathBuf::from(new_current),
        previous_current: previous_current.map(PathBuf::from),
        legacy_entrypoints: Vec::new(),
        created_entrypoints: Vec::new(),
    }
}

#[test]
fn pending_release_journal_keeps_its_declared_byte_limit() {
    assert_eq!(MAX_PENDING_MANIFEST_BYTES, 262_144);
}

#[test]
fn pending_journal_rejects_obsolete_recovery_semantics_before_publication() {
    let root = crate::test_support::fs::unique_temp_path("release-journal-schema");
    let journal = root.join("pending-install.json");
    fs::create_dir_all(&root).expect("create journal schema fixture");
    let mut obsolete = pending("releases/new", None);
    obsolete.schema_version = PENDING_RELEASE_SCHEMA_VERSION.saturating_sub(1);

    let error = write_pending(&journal, &obsolete)
        .expect_err("obsolete entrypoint recovery semantics must fail closed");

    assert!(error
        .to_string()
        .contains("unsupported pending release schema"));
    assert!(fs::symlink_metadata(&journal).is_err());
    fs::remove_dir_all(root).expect("remove journal schema fixture");
}

#[test]
fn managed_current_targets_require_exactly_one_release_generation_component() {
    assert!(is_managed_current_target(Path::new("releases/generation")));
    for target in [
        "generation",
        "foreign/generation",
        "releases",
        "releases/generation/extra",
        "/releases/generation",
    ] {
        assert!(
            !is_managed_current_target(Path::new(target)),
            "unmanaged target was accepted: {target}"
        );
    }
}

#[test]
fn pending_journal_rejects_an_unmanaged_new_or_previous_target() {
    assert!(validate_pending_targets(&pending("foreign/new", None)).is_err());
    assert!(validate_pending_targets(&pending("releases/new", Some("foreign/previous"))).is_err());
    assert!(validate_pending_targets(&pending("releases/new", Some("releases/previous"))).is_ok());

    let mut inconsistent = pending("releases/new", None);
    inconsistent.generation = "different-generation".to_string();
    assert!(validate_pending_targets(&inconsistent).is_err());

    let mut unmanaged_binary = pending("releases/new", None);
    unmanaged_binary
        .legacy_entrypoints
        .push("../outside".to_string());
    assert!(validate_pending_targets(&unmanaged_binary).is_err());

    let mut duplicate_binary = pending("releases/new", None);
    duplicate_binary
        .legacy_entrypoints
        .push("unixnotis-daemon".to_string());
    duplicate_binary
        .created_entrypoints
        .push("unixnotis-daemon".to_string());
    assert!(validate_pending_targets(&duplicate_binary).is_err());
}

#[test]
fn pending_runtime_rollback_state_distinguishes_fresh_and_prior_installs() {
    let root = crate::test_support::fs::unique_temp_path("release-journal-runtime-rollback");
    let paths = paths(&root);
    let journal = paths
        .installed_pending_manifest()
        .expect("pending journal path");
    fs::create_dir_all(journal.parent().expect("journal parent")).expect("create journal parent");

    write_pending(&journal, &pending("releases/new", None)).expect("write fresh journal");
    assert!(
        !pending_release_has_runtime_rollback(&paths).expect("inspect fresh journal"),
        "a fresh install has no prior runtime to restart"
    );

    write_pending(
        &journal,
        &pending("releases/new", Some("releases/previous")),
    )
    .expect("write upgrade journal");
    assert!(
        pending_release_has_runtime_rollback(&paths).expect("inspect upgrade journal"),
        "an upgrade must retain prior runtime recovery"
    );
    fs::remove_dir_all(root).expect("remove runtime rollback fixture");
}

#[test]
fn pending_journal_inspection_propagates_non_missing_filesystem_errors() {
    let root = crate::test_support::fs::unique_temp_path("release-journal-read-error");
    fs::create_dir_all(&root).expect("create journal fixture");
    let journal = root.join("pending-install.json");
    fs::create_dir(&journal).expect("create invalid journal directory");

    assert!(
        read_pending(&journal).is_err(),
        "an invalid journal object must not become an absent transaction"
    );
    fs::remove_dir_all(root).expect("remove journal error fixture");
}

#[test]
fn entrypoint_preparation_rejects_special_objects_and_inspection_errors() {
    let root = crate::test_support::fs::unique_temp_path("release-entrypoint-invalid");
    let paths = paths(&root);
    fs::create_dir_all(&paths.bin_dir).expect("create entrypoint directory");
    fs::create_dir(paths.bin_dir.join("directory-entry")).expect("create invalid entrypoint");

    let special_error =
        plan_entrypoint_changes(&paths, &["directory-entry".to_string()], "test-generation")
            .expect_err("directory entrypoint must fail closed");
    assert!(special_error
        .to_string()
        .contains("not a regular file or managed link"));

    let oversized_name = "x".repeat(4_096);
    let inspection_error = plan_entrypoint_changes(&paths, &[oversized_name], "test-generation")
        .expect_err("an entrypoint inspection error must not become a missing file");
    assert!(inspection_error.to_string().contains("inspect"));
    fs::remove_dir_all(root).expect("remove invalid entrypoint fixture");
}
