use std::fs;
use std::os::unix::fs::PermissionsExt;

use crate::detect::Detection;
use crate::model::ActionMode;
use crate::service_manager::ServiceManager;

use super::super::super::service::{remove_service_artifact, write_service_artifact};
use super::super::support::{test_context, test_paths, test_root};

// These tests verify the actual filesystem shape rendered by each non-systemd backend
// They are intentionally separate from command tests so artifact regressions are easier to audit

#[test]
fn write_and_remove_dinit_artifacts_preserves_boot_symlink_contract() {
    let root = test_root("install-service-dinit-artifacts");
    let paths = test_paths(&root);
    let detection = Detection {
        owner: None,
        daemons: Vec::new(),
    };
    let ctx = test_context(&detection, &paths, ActionMode::Install);
    let manager = ServiceManager::dinit_user(root.join("home").join(".config").join("dinit.d"));
    let artifacts = manager.artifacts(&paths.bin_dir);

    for artifact in &artifacts {
        // The full backend artifact list is written so link and parent ordering is covered
        write_service_artifact(&ctx, artifact).expect("dinit artifact should be written");
    }

    // dinit persistence is installer-owned through boot.d instead of dinitctl enable
    let service_path = root
        .join("home")
        .join(".config")
        .join("dinit.d")
        .join("unixnotis-daemon");
    let boot_link = root
        .join("home")
        .join(".config")
        .join("dinit.d")
        .join("boot.d")
        .join("unixnotis-daemon");
    assert_eq!(
        fs::read_to_string(&service_path).expect("read dinit service"),
        format!(
            "type = process\ncommand = {}/unixnotis-daemon\nrestart = on-failure\n",
            paths.bin_dir.display()
        )
    );
    assert_eq!(
        fs::read_link(&boot_link).expect("read dinit boot link"),
        std::path::Path::new("../unixnotis-daemon")
    );

    // The link is relative because dinit service directories are user-movable config trees
    for artifact in artifacts.iter().rev() {
        // Reverse removal checks that child links go before parent directories
        remove_service_artifact(artifact).expect("dinit artifact should be removed");
    }

    assert!(fs::symlink_metadata(&boot_link).is_err());
    assert!(fs::symlink_metadata(&service_path).is_err());
    assert!(fs::symlink_metadata(manager.artifact_root().join("boot.d")).is_err());
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn write_and_remove_runit_artifacts_preserves_directory_contract() {
    let root = test_root("install-service-runit-artifacts");
    let paths = test_paths(&root);
    let detection = Detection {
        owner: None,
        daemons: Vec::new(),
    };
    let ctx = test_context(&detection, &paths, ActionMode::Install);
    let manager = ServiceManager::runit_user(root.join("home").join(".config").join("service"));
    let artifacts = manager.artifacts(&paths.bin_dir);
    // Include one missing optional variable to prove envdir stale cleanup writes an empty file
    let env_vars = [
        ("WAYLAND_DISPLAY", "wayland-1".to_string()),
        ("XDG_RUNTIME_DIR", "/run/user/1000".to_string()),
    ];
    let env_names = ["WAYLAND_DISPLAY", "DISPLAY", "XDG_RUNTIME_DIR"];
    let env_artifacts = manager.environment_sync_artifacts(&env_names, &env_vars);

    for artifact in artifacts.iter().chain(env_artifacts.iter()) {
        // Runit combines steady service files with envdir artifacts written during env sync
        write_service_artifact(&ctx, artifact).expect("runit artifact should be written");
    }

    let service_dir = root
        .join("home")
        .join(".config")
        .join("service")
        .join("unixnotis-daemon");
    let run_path = service_dir.join("run");
    let marker_path = service_dir.join(".unixnotis-managed");
    let wayland_env = service_dir.join("env").join("WAYLAND_DISPLAY");
    let display_env = service_dir.join("env").join("DISPLAY");
    // The marker is the proof required before recursive uninstall can touch the directory
    assert_eq!(
        fs::read_to_string(&marker_path).expect("read runit marker"),
        "unixnotis\n"
    );
    assert_eq!(
        fs::read_to_string(&run_path).expect("read runit run script"),
        format!(
            "#!/bin/sh\n\
             PATH='/usr/local/sbin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin'; export PATH\n\
             exec chpst -e ./env '{}'\n",
            paths.bin_dir.join("unixnotis-daemon").display()
        )
    );
    // The run script must stay executable regardless of the test process umask
    assert_eq!(
        fs::metadata(&run_path)
            .expect("run script metadata")
            .permissions()
            .mode()
            & 0o777,
        0o755
    );
    assert_eq!(
        fs::read_to_string(&wayland_env).expect("read wayland envdir file"),
        "wayland-1\n"
    );
    assert_eq!(
        fs::read_to_string(&display_env).expect("read stale display envdir file"),
        ""
    );
    // Envdir values are private session data and should not be world-readable
    assert_eq!(
        fs::metadata(&wayland_env)
            .expect("envdir metadata")
            .permissions()
            .mode()
            & 0o777,
        0o600
    );

    for artifact in artifacts.iter().rev() {
        // Envdir cleanup is intentionally left to separate env artifacts
        remove_service_artifact(artifact).expect("runit artifact should be removed");
    }

    assert!(fs::symlink_metadata(&service_dir).is_err());
    assert!(manager.artifact_root().exists());
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn write_and_remove_s6_artifacts_preserves_default_bundle_membership() {
    let root = test_root("install-service-s6-artifacts");
    let paths = test_paths(&root);
    let detection = Detection {
        owner: None,
        daemons: Vec::new(),
    };
    let ctx = test_context(&detection, &paths, ActionMode::Install);
    let manager = ServiceManager::s6_user(root.join("s6"), root.join("run").join("s6-rc"));
    let default_type = root.join("s6").join("sv").join("default").join("type");
    fs::create_dir_all(default_type.parent().expect("default type parent"))
        .expect("default bundle dir");
    fs::write(&default_type, "bundle\n").expect("seed user default bundle");
    let artifacts = manager.artifacts(&paths.bin_dir);

    for artifact in &artifacts {
        // s6 needs both a longrun source and membership in the user's default bundle
        write_service_artifact(&ctx, artifact).expect("s6 artifact should be written");
    }

    let service_dir = root.join("s6").join("sv").join("unixnotis-daemon");
    let default_member = root
        .join("s6")
        .join("sv")
        .join("default")
        .join("contents.d")
        .join("unixnotis-daemon");
    // s6-rc reads source directories, so the service type file is part of the contract
    assert_eq!(
        fs::read_to_string(service_dir.join(".unixnotis-managed")).expect("read marker"),
        "unixnotis\n"
    );
    assert_eq!(
        fs::read_to_string(service_dir.join("type")).expect("read type"),
        "longrun\n"
    );
    assert_eq!(
        fs::read_to_string(service_dir.join("run")).expect("read run"),
        format!(
            "#!/bin/sh\n\
             PATH='/usr/local/sbin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin'; export PATH\n\
             exec s6-envdir ./env '{}'\n",
            paths.bin_dir.join("unixnotis-daemon").display()
        )
    );
    assert!(default_member.exists());

    for artifact in artifacts.iter().rev() {
        // Removing the service source must not remove the user-owned default bundle directory
        remove_service_artifact(artifact).expect("s6 artifact should be removed");
    }

    // Only the membership file is removed from default; the bundle itself remains user-owned
    assert!(!service_dir.exists());
    assert!(!default_member.exists());
    assert!(root.join("s6").join("sv").join("default").exists());
    let _ = fs::remove_dir_all(&root);
}
