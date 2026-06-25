use std::fs;
use std::os::unix::fs::{symlink, PermissionsExt};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use crate::detect::Detection;
use crate::model::ActionMode;
use crate::paths::InstallPaths;
use crate::service_manager::{ServiceArtifact, ServiceArtifactKind};

use super::super::service::{
    install_service, remove_service_artifact, service_start_mode_from_enabled,
    write_service_artifact, ServiceStartMode,
};
use super::support::{test_context, test_paths, test_root};

#[test]
fn install_service_skips_rewrite_when_unit_is_already_current() {
    let root = test_root("install-service-unchanged");
    let paths = test_paths(&root);
    fs::create_dir_all(paths.service.artifact_root()).expect("make service artifact dir");
    let expected = expected_primary_artifact_contents(&paths);
    fs::write(paths.service.primary_artifact_path(), &expected).expect("write current artifact");

    let detection = Detection {
        owner: None,
        daemons: Vec::new(),
    };
    let mut ctx = test_context(&detection, &paths, ActionMode::Install);
    let reload_required = Arc::new(AtomicBool::new(true));
    ctx.service_reload_required = reload_required.clone();

    install_service(&mut ctx).expect("install service should succeed");

    assert_eq!(
        fs::read_to_string(paths.service.primary_artifact_path()).expect("read service artifact"),
        expected
    );
    assert!(!reload_required.load(Ordering::Acquire));

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn install_service_marks_reload_when_unit_changes() {
    let root = test_root("install-service-changed");
    let paths = test_paths(&root);
    fs::create_dir_all(paths.service.artifact_root()).expect("make service artifact dir");
    fs::write(
        paths.service.primary_artifact_path(),
        "[Unit]\nDescription=old\n",
    )
    .expect("write old service artifact");

    let detection = Detection {
        owner: None,
        daemons: Vec::new(),
    };
    let mut ctx = test_context(&detection, &paths, ActionMode::Install);
    let reload_required = Arc::new(AtomicBool::new(false));
    ctx.service_reload_required = reload_required.clone();

    install_service(&mut ctx).expect("install service should succeed");

    assert!(reload_required.load(Ordering::Acquire));
    assert_eq!(
        fs::read_to_string(paths.service.primary_artifact_path()).expect("read service artifact"),
        expected_primary_artifact_contents(&paths)
    );

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn service_start_mode_uses_start_for_enabled_reinstalls() {
    assert_eq!(
        service_start_mode_from_enabled(Some(true)),
        ServiceStartMode::StartOnly
    );
    assert_eq!(
        service_start_mode_from_enabled(Some(false)),
        ServiceStartMode::EnableAndStart
    );
    assert_eq!(
        service_start_mode_from_enabled(None),
        ServiceStartMode::EnableAndStart
    );
}

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
        path: root.join("run"),
        kind: ServiceArtifactKind::ExecutableFile,
        contents: Some("#!/bin/sh\nexec unixnotis-daemon\n".to_string()),
        mode: Some(0o755),
    };

    let changed = write_service_artifact(&ctx, &artifact).expect("script should be written");

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
    symlink(&foreign_target, &unsafe_file_path).expect("create unsafe file link");
    let unsafe_file_artifact = ServiceArtifact {
        path: unsafe_file_path.clone(),
        kind: ServiceArtifactKind::File,
        contents: Some("new contents".to_string()),
        mode: None,
    };

    let err = write_service_artifact(&ctx, &unsafe_file_artifact)
        .expect_err("symlink file artifact is unsafe");

    assert!(err.to_string().contains("cannot replace symlink"));
    assert_eq!(
        fs::read_link(&unsafe_file_path).expect("unsafe link should remain"),
        foreign_target
    );

    let path = root.join("service-link");
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
fn uninstall_does_not_follow_service_symlink() {
    let root = test_root("install-service-remove-matching-symlink");
    fs::create_dir_all(&root).expect("make root");
    let target = root.join("target");
    let link = root.join("service-link");
    fs::write(&target, "target").expect("write target");
    symlink(&target, &link).expect("create link");
    let artifact = ServiceArtifact {
        path: link,
        kind: ServiceArtifactKind::Symlink {
            target: target.clone(),
        },
        contents: None,
        mode: None,
    };

    remove_service_artifact(&artifact).expect("link should be removed");

    assert!(fs::symlink_metadata(&artifact.path).is_err());
    assert!(target.exists());
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn uninstall_does_not_remove_non_matching_symlink() {
    let root = test_root("install-service-keep-foreign-symlink");
    fs::create_dir_all(&root).expect("make root");
    let actual_target = root.join("actual-target");
    let expected_target = root.join("expected-target");
    let link = root.join("service-link");
    fs::write(&actual_target, "actual").expect("write actual target");
    fs::write(&expected_target, "expected").expect("write expected target");
    symlink(&actual_target, &link).expect("create link");
    let artifact = ServiceArtifact {
        path: link.clone(),
        kind: ServiceArtifactKind::Symlink {
            target: expected_target.clone(),
        },
        contents: None,
        mode: None,
    };

    let err = remove_service_artifact(&artifact).expect_err("foreign link should not be removed");

    assert!(err.to_string().contains("refusing to remove symlink"));
    assert_eq!(
        fs::read_link(&link).expect("link should remain"),
        actual_target
    );
    assert!(expected_target.exists());
    let _ = fs::remove_dir_all(&root);
}

fn expected_primary_artifact_contents(paths: &InstallPaths) -> String {
    paths
        .service
        .artifacts(&paths.bin_dir)
        .into_iter()
        .find(|artifact| artifact.path == paths.service.primary_artifact_path())
        .and_then(|artifact| artifact.contents)
        .expect("primary artifact should have rendered contents")
}
