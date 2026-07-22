use std::fs;
use std::os::unix::fs::{symlink, FileTypeExt, PermissionsExt};
use std::os::unix::net::UnixListener;

use crate::detect::Detection;
use crate::model::ActionMode;
use crate::service_manager::{ServiceArtifact, ServiceArtifactKind};

use super::super::super::service::artifacts::remove_service_artifact;
use super::super::super::service::files::{current_mode, ensure_regular_artifact_file_path};
use super::super::super::service::symlinks::{remove_service_symlink, write_service_symlink};
use super::super::super::service::write_service_artifact;
use super::super::support::{test_context, test_paths, test_root};

// Write-path tests cover the low-level artifact writer before backend-specific lists use it
// The cases here focus on filesystem shape, permissions, and symlink refusal

#[test]
fn write_service_artifact_creates_directory_artifact() {
    let root = test_root("install-service-directory-artifact");
    let paths = test_paths(&root);
    let detection = Detection {
        owner: None,
        daemons: Vec::new(),
    };
    let ctx = test_context(&detection, &paths, ActionMode::Install);
    let artifact = ServiceArtifact {
        // Plain directories represent parent containers, not recursive ownership
        path: root.join("service-dir"),
        kind: ServiceArtifactKind::Directory,
        contents: None,
        mode: None,
    };

    let changed = write_service_artifact(&ctx, &artifact).expect("directory should be created");

    assert!(changed);
    assert!(artifact.path.is_dir());
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn write_service_artifact_sets_executable_file_mode() {
    let root = test_root("install-service-executable-artifact");
    let paths = test_paths(&root);
    let detection = Detection {
        owner: None,
        daemons: Vec::new(),
    };
    let ctx = test_context(&detection, &paths, ActionMode::Install);
    let artifact = ServiceArtifact {
        // Executable files cover runit and s6 run scripts that must ignore process umask
        path: root.join("run"),
        kind: ServiceArtifactKind::ExecutableFile,
        contents: Some("#!/bin/sh\nexec unixnotis-daemon\n".to_string()),
        mode: Some(0o755),
    };

    let changed = write_service_artifact(&ctx, &artifact).expect("script should be written");

    // The writer returns changed for both new files and content replacement
    assert!(changed);
    assert_eq!(
        fs::read_to_string(&artifact.path).expect("read script"),
        "#!/bin/sh\nexec unixnotis-daemon\n"
    );
    assert_eq!(
        fs::metadata(&artifact.path)
            .expect("script metadata")
            .permissions()
            .mode()
            & 0o777,
        0o755
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn write_service_artifact_reports_executable_mode_changes() {
    let root = test_root("install-service-executable-mode-change");
    let paths = test_paths(&root);
    let detection = Detection {
        owner: None,
        daemons: Vec::new(),
    };
    let ctx = test_context(&detection, &paths, ActionMode::Install);
    let artifact = ServiceArtifact {
        path: root.join("run"),
        kind: ServiceArtifactKind::ExecutableFile,
        contents: Some("#!/bin/sh\nexec unixnotis-daemon\n".to_string()),
        mode: Some(0o755),
    };
    fs::create_dir_all(&root).expect("make service root");
    fs::write(
        &artifact.path,
        artifact.contents.as_ref().expect("script contents"),
    )
    .expect("seed script");
    fs::set_permissions(&artifact.path, fs::Permissions::from_mode(0o644))
        .expect("seed non-executable mode");

    let changed = write_service_artifact(&ctx, &artifact).expect("mode drift should be fixed");

    // Matching bytes but wrong mode still require a reload/start-visible artifact change
    assert!(changed);
    assert_eq!(
        fs::metadata(&artifact.path)
            .expect("script metadata")
            .permissions()
            .mode()
            & 0o777,
        0o755
    );

    let changed_again =
        write_service_artifact(&ctx, &artifact).expect("matching script should stay quiet");

    // Reinstall should not look dirty once both contents and mode already match
    assert!(!changed_again);
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn write_service_artifact_preserves_mode_when_no_mode_is_requested() {
    let root = test_root("install-service-preserve-file-mode");
    let paths = test_paths(&root);
    let detection = Detection {
        owner: None,
        daemons: Vec::new(),
    };
    let ctx = test_context(&detection, &paths, ActionMode::Install);
    let artifact = ServiceArtifact {
        path: root.join("service.conf"),
        kind: ServiceArtifactKind::File,
        contents: Some("new contents\n".to_string()),
        mode: None,
    };
    fs::create_dir_all(&root).expect("make service root");
    fs::write(&artifact.path, "old contents\n").expect("seed service file");
    fs::set_permissions(&artifact.path, fs::Permissions::from_mode(0o640))
        .expect("seed service file mode");

    let changed = write_service_artifact(&ctx, &artifact).expect("service file should update");

    assert!(changed);
    assert_eq!(
        fs::read_to_string(&artifact.path).expect("read updated service file"),
        "new contents\n"
    );
    assert_eq!(
        fs::metadata(&artifact.path)
            .expect("service file metadata")
            .permissions()
            .mode()
            & 0o777,
        0o640
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn write_managed_directory_artifact_creates_ownership_marker() {
    let root = test_root("install-service-managed-directory");
    let paths = test_paths(&root);
    let detection = Detection {
        owner: None,
        daemons: Vec::new(),
    };
    let ctx = test_context(&detection, &paths, ActionMode::Install);
    let artifact = ServiceArtifact {
        // Managed directories are recursive uninstall roots and require an ownership marker
        path: root.join("managed-service"),
        kind: ServiceArtifactKind::ManagedDirectory,
        contents: None,
        mode: None,
    };

    let changed = write_service_artifact(&ctx, &artifact).expect("managed dir should be created");

    // Managed directory writes seed the marker that later authorizes recursive cleanup
    assert!(changed);
    assert_eq!(
        fs::read_to_string(artifact.path.join(".unixnotis-managed")).expect("read marker"),
        "unixnotis\n"
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn write_managed_directory_rejects_unmarked_existing_directory() {
    let root = test_root("install-service-unmarked-directory");
    let paths = test_paths(&root);
    let detection = Detection {
        owner: None,
        daemons: Vec::new(),
    };
    let ctx = test_context(&detection, &paths, ActionMode::Install);
    let service_dir = root.join("preexisting-service");
    fs::create_dir_all(&service_dir).expect("make preexisting service dir");
    // A foreign file makes the directory look like a pre-existing user or manager service
    fs::write(service_dir.join("foreign"), "do not delete").expect("foreign file");
    let artifact = ServiceArtifact {
        path: service_dir.clone(),
        kind: ServiceArtifactKind::ManagedDirectory,
        contents: None,
        mode: None,
    };

    let err = write_service_artifact(&ctx, &artifact).expect_err("unmarked dir is unsafe");

    // The foreign file should survive because the installer never adopted this directory
    assert!(err.to_string().contains("refusing to manage unmarked"));
    assert!(service_dir.join("foreign").exists());
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn write_service_artifact_rejects_symlink_parent_component() {
    let root = test_root("install-service-symlink-parent");
    let paths = test_paths(&root);
    let detection = Detection {
        owner: None,
        daemons: Vec::new(),
    };
    let ctx = test_context(&detection, &paths, ActionMode::Install);
    let target = root.join("target");
    let symlink_parent = root.join("linked-parent");
    fs::create_dir_all(&target).expect("make target dir");
    // Parent symlinks are more dangerous than final-path symlinks because writes can be redirected
    symlink(&target, &symlink_parent).expect("create parent link");
    let artifact = ServiceArtifact {
        path: symlink_parent.join("service-file"),
        kind: ServiceArtifactKind::File,
        contents: Some("contents".to_string()),
        mode: None,
    };

    let err = write_service_artifact(&ctx, &artifact).expect_err("symlink parent is unsafe");

    // The target directory proves the writer did not follow the linked parent
    assert!(format!("{err:#}").contains("refusing unsafe service directory path"));
    assert!(!target.join("service-file").exists());
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn write_service_artifact_rejects_non_matching_symlink_target() {
    let root = test_root("install-service-symlink-target-reject");
    let paths = test_paths(&root);
    let detection = Detection {
        owner: None,
        daemons: Vec::new(),
    };
    let ctx = test_context(&detection, &paths, ActionMode::Install);
    fs::create_dir_all(&root).expect("make root");
    let existing_target = root.join("existing-target");
    let expected_target = root.join("expected-target");
    let link_path = root.join("service-link");
    fs::write(&existing_target, "existing").expect("write existing target");
    fs::write(&expected_target, "expected").expect("write expected target");
    // A symlink with a different target could belong to another manager or a user hand edit
    symlink(&existing_target, &link_path).expect("create existing service link");
    let artifact = ServiceArtifact {
        path: link_path.clone(),
        kind: ServiceArtifactKind::Symlink {
            target: expected_target,
        },
        contents: None,
        mode: None,
    };

    let err = write_service_artifact(&ctx, &artifact).expect_err("foreign symlink is not replaced");

    // The existing link target stays intact, matching uninstall's conservative ownership check
    assert!(err.to_string().contains("cannot replace service symlink"));
    assert_eq!(
        fs::read_link(&link_path).expect("service link should remain"),
        existing_target
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn write_shared_service_file_refuses_to_overwrite_user_content() {
    let root = test_root("install-service-shared-file");
    let paths = test_paths(&root);
    let detection = Detection {
        owner: None,
        daemons: Vec::new(),
    };
    let ctx = test_context(&detection, &paths, ActionMode::Install);
    let artifact = ServiceArtifact {
        // Shared files can seed manager layout but cannot overwrite existing user state
        path: root.join("default").join("type"),
        kind: ServiceArtifactKind::SharedFile {
            created_marker: Some(root.join("default").join(".unixnotis-created-type")),
        },
        contents: Some("bundle\n".to_string()),
        mode: Some(0o644),
    };

    let changed = write_service_artifact(&ctx, &artifact).expect("missing shared file is seeded");

    assert!(changed);
    assert_eq!(
        fs::read_to_string(&artifact.path).expect("read shared file"),
        "bundle\n"
    );

    let unchanged =
        write_service_artifact(&ctx, &artifact).expect("matching shared file is accepted");

    assert!(!unchanged);
    fs::set_permissions(&artifact.path, fs::Permissions::from_mode(0o600))
        .expect("seed shared file mode drift");

    let mode_fixed =
        write_service_artifact(&ctx, &artifact).expect("matching shared file mode is repaired");

    // Permission-only repair keeps shared contents untouched and remains an unchanged write
    assert!(!mode_fixed);
    assert_eq!(
        fs::metadata(&artifact.path)
            .expect("shared file metadata")
            .permissions()
            .mode()
            & 0o777,
        0o644
    );
    fs::write(&artifact.path, "longrun\n").expect("seed incompatible shared file");

    let err =
        write_service_artifact(&ctx, &artifact).expect_err("shared file must not be overwritten");

    assert!(err.to_string().contains("refusing to overwrite"));
    assert_eq!(
        fs::read_to_string(&artifact.path).expect("read shared file after failure"),
        "longrun\n"
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn write_shared_service_file_preserves_an_exact_unowned_file() {
    let root = test_root("install-service-shared-unowned-file");
    let paths = test_paths(&root);
    let detection = Detection {
        owner: None,
        daemons: Vec::new(),
    };
    let ctx = test_context(&detection, &paths, ActionMode::Install);
    let marker = root.join("default").join(".unixnotis-created-type");
    let artifact = ServiceArtifact {
        path: root.join("default").join("type"),
        kind: ServiceArtifactKind::SharedFile {
            created_marker: Some(marker.clone()),
        },
        contents: Some("bundle\n".to_string()),
        mode: Some(0o644),
    };
    fs::create_dir_all(artifact.path.parent().expect("shared file parent"))
        .expect("create shared file parent");
    fs::write(&artifact.path, "bundle\n").expect("seed exact unmarked shared file");

    let unchanged = write_service_artifact(&ctx, &artifact).expect("accept exact unowned file");

    assert!(!unchanged);
    assert!(!marker.exists());
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn write_shared_service_file_rolls_back_when_the_marker_conflicts() {
    let root = test_root("install-service-shared-marker-conflict");
    let paths = test_paths(&root);
    let detection = Detection {
        owner: None,
        daemons: Vec::new(),
    };
    let ctx = test_context(&detection, &paths, ActionMode::Install);
    let marker = root.join("default").join(".unixnotis-created-type");
    let artifact = ServiceArtifact {
        path: root.join("default").join("type"),
        kind: ServiceArtifactKind::SharedFile {
            created_marker: Some(marker.clone()),
        },
        contents: Some("bundle\n".to_string()),
        mode: Some(0o644),
    };
    fs::create_dir_all(marker.parent().expect("marker parent")).expect("create shared file parent");
    fs::write(&marker, "foreign\n").expect("seed foreign marker");

    let error = write_service_artifact(&ctx, &artifact)
        .expect_err("conflicting marker should reject the pair");

    assert!(error.to_string().contains("refusing to overwrite"));
    assert!(!artifact.path.exists());
    assert_eq!(
        fs::read_to_string(marker).expect("read foreign marker"),
        "foreign\n"
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn remove_shared_service_file_only_removes_marker_owned_file() {
    let root = test_root("install-service-shared-file-remove");
    let paths = test_paths(&root);
    let detection = Detection {
        owner: None,
        daemons: Vec::new(),
    };
    let ctx = test_context(&detection, &paths, ActionMode::Install);
    let marker = root.join("default").join(".unixnotis-created-type");
    let artifact = ServiceArtifact {
        path: root.join("default").join("type"),
        kind: ServiceArtifactKind::SharedFile {
            created_marker: Some(marker.clone()),
        },
        contents: Some("bundle\n".to_string()),
        mode: Some(0o644),
    };

    write_service_artifact(&ctx, &artifact).expect("missing shared file is seeded");
    fs::create_dir_all(root.join("default").join("contents.d")).expect("shared child dir");

    let removed = remove_service_artifact(&artifact).expect("marker-owned file should remove");

    assert!(removed);
    assert!(!artifact.path.exists());
    assert!(!marker.exists());
    assert!(!root.join("default").exists());
    fs::create_dir_all(artifact.path.parent().expect("shared file parent")).expect("parent dir");
    fs::write(&artifact.path, "bundle\n").expect("seed user shared file");

    let skipped = remove_service_artifact(&artifact).expect("unmarked shared file should stay");

    assert!(!skipped);
    assert_eq!(
        fs::read_to_string(&artifact.path).expect("read unmarked shared file"),
        "bundle\n"
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn remove_shared_service_file_preserves_marker_file_after_user_edit() {
    let root = test_root("install-service-shared-file-edited-remove");
    let paths = test_paths(&root);
    let detection = Detection {
        owner: None,
        daemons: Vec::new(),
    };
    let ctx = test_context(&detection, &paths, ActionMode::Install);
    let marker = root.join("default").join(".unixnotis-created-type");
    let artifact = ServiceArtifact {
        path: root.join("default").join("type"),
        kind: ServiceArtifactKind::SharedFile {
            created_marker: Some(marker.clone()),
        },
        contents: Some("bundle\n".to_string()),
        mode: Some(0o644),
    };
    write_service_artifact(&ctx, &artifact).expect("missing shared file is seeded");
    fs::write(&artifact.path, "longrun\n").expect("simulate user changing shared setup file");

    let removed = remove_service_artifact(&artifact).expect("edited shared file should be skipped");

    // The marker alone is not enough proof once the shared file contents no longer match
    assert!(!removed);
    assert_eq!(
        fs::read_to_string(&artifact.path).expect("edited shared file should remain"),
        "longrun\n"
    );
    assert_eq!(
        fs::read_to_string(&marker).expect("marker remains for manual cleanup context"),
        "unixnotis\n"
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn remove_shared_service_file_handles_missing_and_invalid_paths_safely() {
    let root = test_root("install-service-shared-file-invalid-remove");
    let marker = root.join(".unixnotis-created-type");
    fs::create_dir_all(&root).expect("make shared service root");
    fs::write(&marker, "unixnotis\n").expect("write valid ownership marker");
    let missing = ServiceArtifact {
        path: root.join("missing-type"),
        kind: ServiceArtifactKind::SharedFile {
            created_marker: Some(marker.clone()),
        },
        contents: Some("bundle\n".to_string()),
        mode: Some(0o644),
    };

    let removed = remove_service_artifact(&missing).expect("missing shared file should be safe");

    assert!(!removed);
    assert!(marker.exists());

    let blocked_parent = root.join("blocked-parent");
    fs::write(&blocked_parent, "regular file").expect("write blocking parent");
    let invalid = ServiceArtifact {
        path: blocked_parent.join("type"),
        kind: ServiceArtifactKind::SharedFile {
            created_marker: Some(marker.clone()),
        },
        contents: Some("bundle\n".to_string()),
        mode: Some(0o644),
    };

    let error = remove_service_artifact(&invalid)
        .expect_err("filesystem errors must not look like missing shared files");

    assert!(format!("{error:#}").contains("failed to remove"));
    assert!(marker.exists());

    let directory_artifact = ServiceArtifact {
        path: root.join("directory-type"),
        kind: ServiceArtifactKind::SharedFile {
            created_marker: Some(marker),
        },
        contents: Some("bundle\n".to_string()),
        mode: Some(0o644),
    };
    fs::create_dir_all(&directory_artifact.path).expect("make invalid shared directory");

    let error = remove_service_artifact(&directory_artifact)
        .expect_err("a directory must never be removed as a shared file");

    assert!(
        format!("{error:#}").contains("non-regular file target"),
        "unexpected shared directory error: {error:#}"
    );
    assert!(directory_artifact.path.is_dir());
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn remove_shared_service_file_preserves_nonempty_layout_directories() {
    let root = test_root("install-service-shared-file-nonempty-layout");
    let paths = test_paths(&root);
    let detection = Detection {
        owner: None,
        daemons: Vec::new(),
    };
    let ctx = test_context(&detection, &paths, ActionMode::Install);
    let marker = root.join("default").join(".unixnotis-created-type");
    let artifact = ServiceArtifact {
        path: root.join("default").join("type"),
        kind: ServiceArtifactKind::SharedFile {
            created_marker: Some(marker.clone()),
        },
        contents: Some("bundle\n".to_string()),
        mode: Some(0o644),
    };
    write_service_artifact(&ctx, &artifact).expect("seed marker-owned shared file");
    let foreign = root.join("default").join("contents.d").join("foreign");
    fs::create_dir_all(foreign.parent().expect("foreign file parent"))
        .expect("make shared contents directory");
    fs::write(&foreign, "keep").expect("seed foreign bundle member");

    let removed = remove_service_artifact(&artifact).expect("owned file removal should succeed");

    assert!(removed);
    assert!(!artifact.path.exists());
    assert!(!marker.exists());
    assert_eq!(
        fs::read_to_string(&foreign).expect("foreign file remains"),
        "keep"
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn remove_shared_service_file_reports_invalid_layout_cleanup_shape() {
    let root = test_root("install-service-shared-file-invalid-layout");
    let paths = test_paths(&root);
    let detection = Detection {
        owner: None,
        daemons: Vec::new(),
    };
    let ctx = test_context(&detection, &paths, ActionMode::Install);
    let marker = root.join("default").join(".unixnotis-created-type");
    let artifact = ServiceArtifact {
        path: root.join("default").join("type"),
        kind: ServiceArtifactKind::SharedFile {
            created_marker: Some(marker.clone()),
        },
        contents: Some("bundle\n".to_string()),
        mode: Some(0o644),
    };
    write_service_artifact(&ctx, &artifact).expect("seed marker-owned shared file");
    let invalid_contents_dir = root.join("default").join("contents.d");
    // A regular file at the cleanup-only directory path is foreign and must surface an error
    fs::write(&invalid_contents_dir, "foreign").expect("seed invalid layout path");

    let error = remove_service_artifact(&artifact)
        .expect_err("invalid layout shape must not be reported as clean removal");

    assert!(format!("{error:#}").contains("failed to remove"));
    assert_eq!(
        fs::read_to_string(&invalid_contents_dir).expect("foreign layout file remains"),
        "foreign"
    );
    // Owned files were removed before the cleanup-only layout error was discovered
    assert!(!artifact.path.exists());
    assert!(!marker.exists());
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn regular_service_file_shape_checks_distinguish_missing_directory_and_io_errors() {
    let root = test_root("install-service-file-shape-checks");
    let missing = root.join("missing");
    fs::create_dir_all(&root).expect("make service root");

    assert!(!ensure_regular_artifact_file_path(&missing).expect("missing path is available"));
    fs::write(&missing, "service").expect("write regular service file");
    assert!(ensure_regular_artifact_file_path(&missing).expect("regular file is replaceable"));

    let directory = root.join("directory");
    fs::create_dir(&directory).expect("make conflicting directory");
    let directory_error = ensure_regular_artifact_file_path(&directory)
        .expect_err("directory conflict must remain distinct");
    assert!(directory_error
        .to_string()
        .contains("cannot replace directory"));

    let blocked_parent = root.join("blocked-parent");
    fs::write(&blocked_parent, "regular file").expect("write blocking parent");
    let io_error = ensure_regular_artifact_file_path(&blocked_parent.join("child"))
        .expect_err("NotADirectory must not be treated as a missing file");
    assert!(format!("{io_error:#}").contains("failed to inspect"));
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn current_mode_propagates_filesystem_errors_other_than_missing_paths() {
    let root = test_root("install-service-current-mode-error");
    fs::create_dir_all(&root).expect("make service root");
    let blocked_parent = root.join("blocked-parent");
    fs::write(&blocked_parent, "regular file").expect("write blocking parent");

    let error = current_mode(&blocked_parent.join("child"))
        .expect_err("NotADirectory must not be treated as an absent mode");

    assert!(format!("{error:#}").contains("failed to inspect"));
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn service_symlink_helpers_distinguish_missing_paths_from_filesystem_errors() {
    let root = test_root("install-service-symlink-errors");
    fs::create_dir_all(&root).expect("make service root");
    let missing = root.join("missing-link");
    let expected_target = root.join("target");

    remove_service_symlink(&missing, &expected_target)
        .expect("missing symlink should make uninstall idempotent");

    let blocked_parent = root.join("blocked-parent");
    fs::write(&blocked_parent, "regular file").expect("write blocking parent");
    let blocked_link = blocked_parent.join("service-link");

    let remove_error = remove_service_symlink(&blocked_link, &expected_target)
        .expect_err("remove must preserve NotADirectory errors");
    assert!(format!("{remove_error:#}").contains("failed to inspect"));

    let write_error = write_service_symlink(&blocked_link, &expected_target)
        .expect_err("write must preserve NotADirectory errors");
    assert!(format!("{write_error:#}").contains("failed to inspect"));
    assert!(blocked_parent.is_file());
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn service_symlink_removal_rejects_a_regular_file_without_deleting_it() {
    let root = test_root("install-service-remove-regular-link");
    let artifact = root.join("service-link");
    fs::create_dir_all(&root).expect("make service root");
    fs::write(&artifact, "user data").expect("write regular artifact");

    let error = remove_service_symlink(&artifact, std::path::Path::new("service"))
        .expect_err("regular artifacts must not be removed as links");

    assert!(format!("{error:#}").contains("refusing to remove non-symlink"));
    assert_eq!(
        fs::read_to_string(&artifact).expect("read preserved artifact"),
        "user data"
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn install_replaces_regular_owned_artifact_but_rejects_unsafe_existing_path() {
    let root = test_root("install-service-owned-replace");
    let paths = test_paths(&root);
    let detection = Detection {
        owner: None,
        daemons: Vec::new(),
    };
    let ctx = test_context(&detection, &paths, ActionMode::Install);

    let owned_path = root.join("owned-service-file");
    fs::create_dir_all(&root).expect("make root");
    // Regular files are installer-owned shapes, so replacing old bytes is expected
    fs::write(&owned_path, "old contents").expect("write old owned file");
    let owned_artifact = ServiceArtifact {
        path: owned_path.clone(),
        kind: ServiceArtifactKind::File,
        contents: Some("new contents".to_string()),
        mode: None,
    };

    let changed =
        write_service_artifact(&ctx, &owned_artifact).expect("owned file should be replaced");

    assert!(changed);
    assert_eq!(
        fs::read_to_string(&owned_path).expect("read replaced file"),
        "new contents"
    );

    let foreign_target = root.join("foreign-target");
    let unsafe_file_path = root.join("unsafe-service-file");
    fs::write(&foreign_target, "new contents").expect("write foreign target");
    // Replacing a symlink file would follow attacker-controlled path state
    symlink(&foreign_target, &unsafe_file_path).expect("create unsafe file link");
    let unsafe_file_artifact = ServiceArtifact {
        path: unsafe_file_path.clone(),
        kind: ServiceArtifactKind::File,
        contents: Some("new contents".to_string()),
        mode: None,
    };

    let err = write_service_artifact(&ctx, &unsafe_file_artifact)
        .expect_err("symlink file artifact is unsafe");

    // The symlink remains intact so the foreign target is not modified through it
    assert!(err.to_string().contains("cannot replace symlink"));
    assert_eq!(
        fs::read_link(&unsafe_file_path).expect("unsafe link should remain"),
        foreign_target
    );

    let path = root.join("service-link");
    // Symlink artifacts are allowed to replace symlinks only, not regular user files
    fs::write(&path, "not a symlink").expect("write regular file");
    let artifact = ServiceArtifact {
        path,
        kind: ServiceArtifactKind::Symlink {
            target: root.join("target"),
        },
        contents: None,
        mode: None,
    };

    let err = write_service_artifact(&ctx, &artifact).expect_err("regular file is not replaced");

    assert!(err.to_string().contains("cannot replace non-symlink"));
    assert_eq!(
        fs::read_to_string(&artifact.path).expect("regular file should remain"),
        "not a symlink"
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn install_replaces_an_oversized_sparse_service_file_without_reading_it() {
    let root = test_root("install-service-oversized-regular-file");
    let paths = test_paths(&root);
    let detection = Detection {
        owner: None,
        daemons: Vec::new(),
    };
    let ctx = test_context(&detection, &paths, ActionMode::Install);
    fs::create_dir_all(&root).expect("create service root");
    let path = root.join("service-file");
    let oversized = fs::File::create(&path).expect("create sparse service file");
    oversized
        .set_len(1_073_741_824)
        .expect("extend sparse service file");
    drop(oversized);
    let artifact = ServiceArtifact {
        path: path.clone(),
        kind: ServiceArtifactKind::File,
        contents: Some("service\n".to_string()),
        mode: None,
    };

    let changed =
        write_service_artifact(&ctx, &artifact).expect("replace oversized regular service file");

    assert!(changed);
    assert_eq!(
        fs::read_to_string(path).expect("read replaced service file"),
        "service\n"
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn write_service_artifact_rejects_socket_artifact_path() {
    let root = test_root("install-service-special-file-reject");
    let paths = test_paths(&root);
    let detection = Detection {
        owner: None,
        daemons: Vec::new(),
    };
    let ctx = test_context(&detection, &paths, ActionMode::Install);
    fs::create_dir_all(&root).expect("make root");
    let socket_path = root.join("service.socket");
    // A Unix socket is a simple special file that must never be read as service text
    let _listener = UnixListener::bind(&socket_path).expect("create socket artifact path");
    let artifact = ServiceArtifact {
        path: socket_path.clone(),
        kind: ServiceArtifactKind::File,
        contents: Some("new contents".to_string()),
        mode: None,
    };

    let err = write_service_artifact(&ctx, &artifact).expect_err("socket path is unsafe");

    // The socket remains untouched and the writer fails before descriptor comparison can block
    assert!(err
        .to_string()
        .contains("cannot replace non-regular service artifact"));
    assert!(fs::symlink_metadata(&socket_path)
        .expect("socket should remain")
        .file_type()
        .is_socket());
    let _ = fs::remove_dir_all(&root);
}
