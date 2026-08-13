use std::fs;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};

use super::super::entrypoints::plan_entrypoint_changes;
use super::super::manifest::entrypoint_target;
use super::super::transaction::{
    pending_release_exists, rollback_pending_release, write_pending, PendingRelease,
};
use crate::paths::InstallPaths;
use crate::service_manager::ServiceManager;

const LEGACY_BINARIES: [&str; 4] = [
    "unixnotis-daemon",
    "unixnotis-popups",
    "unixnotis-center",
    "unixnotis-svg-renderer",
];
const CREATED_BINARIES: [&str; 2] = ["unixnotis-css-validate", "noticenterctl"];

#[derive(Clone, Copy)]
struct CrashBoundary {
    label: &'static str,
    rollback_directory: bool,
    moved: usize,
    linked: usize,
    current_switched: bool,
}

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

#[test]
fn every_entrypoint_crash_boundary_recovers_one_complete_legacy_layout() {
    for boundary in crash_boundaries() {
        let root =
            crate::test_support::fs::unique_temp_path(&format!("release-crash-{}", boundary.label));
        let paths = paths(&root);
        let pending = prepare_journaled_legacy_layout(&paths);
        apply_crash_prefix(&paths, &pending, boundary);

        assert!(
            rollback_pending_release(&paths).expect("recover crash boundary"),
            "{} did not find its durable journal",
            boundary.label
        );
        assert_recovered_legacy_layout(&paths, boundary.label);
        fs::remove_dir_all(root).expect("remove crash recovery fixture");
    }
}

fn crash_boundaries() -> [CrashBoundary; 9] {
    [
        CrashBoundary {
            label: "journal-only",
            rollback_directory: false,
            moved: 0,
            linked: 0,
            current_switched: false,
        },
        CrashBoundary {
            label: "rollback-directory-created",
            rollback_directory: true,
            moved: 0,
            linked: 0,
            current_switched: false,
        },
        CrashBoundary {
            label: "first-legacy-moved",
            rollback_directory: true,
            moved: 1,
            linked: 0,
            current_switched: false,
        },
        CrashBoundary {
            label: "half-legacy-moved",
            rollback_directory: true,
            moved: 2,
            linked: 0,
            current_switched: false,
        },
        CrashBoundary {
            label: "all-legacy-moved",
            rollback_directory: true,
            moved: LEGACY_BINARIES.len(),
            linked: 0,
            current_switched: false,
        },
        CrashBoundary {
            label: "first-link-created",
            rollback_directory: true,
            moved: LEGACY_BINARIES.len(),
            linked: 1,
            current_switched: false,
        },
        CrashBoundary {
            label: "half-links-created",
            rollback_directory: true,
            moved: LEGACY_BINARIES.len(),
            linked: usize::midpoint(LEGACY_BINARIES.len(), CREATED_BINARIES.len()),
            current_switched: false,
        },
        CrashBoundary {
            label: "all-links-created",
            rollback_directory: true,
            moved: LEGACY_BINARIES.len(),
            linked: LEGACY_BINARIES.len() + CREATED_BINARIES.len(),
            current_switched: false,
        },
        CrashBoundary {
            label: "current-switched-before-readiness",
            rollback_directory: true,
            moved: LEGACY_BINARIES.len(),
            linked: LEGACY_BINARIES.len() + CREATED_BINARIES.len(),
            current_switched: true,
        },
    ]
}

fn assert_recovered_legacy_layout(paths: &InstallPaths, boundary: &str) {
    for name in LEGACY_BINARIES {
        assert_eq!(
            fs::read_to_string(paths.bin_dir.join(name)).expect("read restored legacy binary"),
            format!("legacy:{name}"),
            "{boundary} did not restore {name}"
        );
    }
    for name in CREATED_BINARIES {
        assert!(
            fs::symlink_metadata(paths.bin_dir.join(name)).is_err(),
            "{boundary} retained newly created entrypoint {name}"
        );
    }
    assert!(
        fs::symlink_metadata(paths.installed_current_link().expect("current path")).is_err(),
        "{boundary} retained the unready generation"
    );
    assert!(
        !pending_release_exists(paths).expect("inspect recovered journal"),
        "{boundary} retained a completed recovery journal"
    );
}

#[test]
fn recovery_fails_closed_when_legacy_live_and_backup_states_conflict() {
    for conflict in ["both-copies", "neither-copy"] {
        let root =
            crate::test_support::fs::unique_temp_path(&format!("release-conflict-{conflict}"));
        let paths = paths(&root);
        let pending = prepare_journaled_legacy_layout(&paths);
        let name = LEGACY_BINARIES[0];
        let rollback = rollback_bin_dir(&paths, &pending);
        fs::create_dir_all(&rollback).expect("create rollback directory");
        match conflict {
            "both-copies" => {
                fs::write(rollback.join(name), "duplicate backup").expect("write duplicate backup");
            }
            "neither-copy" => {
                fs::remove_file(paths.bin_dir.join(name)).expect("remove legacy live copy");
            }
            _ => unreachable!("the conflict table lists every case"),
        }

        let error = rollback_pending_release(&paths)
            .expect_err("ambiguous rollback state must fail closed");

        assert!(
            error.to_string().contains("binary rollback"),
            "unexpected recovery error for {conflict}: {error:#}"
        );
        assert!(
            pending_release_exists(&paths).expect("retain failed recovery journal"),
            "failed recovery must retain its journal"
        );
        fs::remove_dir_all(root).expect("remove conflict recovery fixture");
    }
}

fn prepare_journaled_legacy_layout(paths: &InstallPaths) -> PendingRelease {
    fs::create_dir_all(&paths.bin_dir).expect("create legacy bin directory");
    for name in LEGACY_BINARIES {
        fs::write(paths.bin_dir.join(name), format!("legacy:{name}")).expect("write legacy binary");
    }
    fs::create_dir_all(
        paths
            .installed_release_root()
            .expect("installed release root"),
    )
    .expect("create release journal directory");
    let binaries = LEGACY_BINARIES
        .into_iter()
        .chain(CREATED_BINARIES)
        .map(str::to_string)
        .collect::<Vec<_>>();
    let pending = plan_entrypoint_changes(paths, &binaries, "test-generation")
        .expect("plan entrypoint changes");
    write_pending(
        &paths
            .installed_pending_manifest()
            .expect("pending journal path"),
        &pending,
    )
    .expect("write durable pending journal");
    pending
}

fn apply_crash_prefix(paths: &InstallPaths, pending: &PendingRelease, boundary: CrashBoundary) {
    let rollback = rollback_bin_dir(paths, pending);
    if boundary.rollback_directory {
        fs::create_dir_all(&rollback).expect("create rollback directory");
    }
    for name in LEGACY_BINARIES.into_iter().take(boundary.moved) {
        fs::rename(paths.bin_dir.join(name), rollback.join(name)).expect("move legacy binary");
    }

    let expected = entrypoint_target();
    for name in LEGACY_BINARIES
        .into_iter()
        .chain(CREATED_BINARIES)
        .take(boundary.linked)
    {
        symlink(expected.join(name), paths.bin_dir.join(name)).expect("create managed entrypoint");
    }
    if boundary.current_switched {
        symlink(
            &pending.new_current,
            paths.installed_current_link().expect("current path"),
        )
        .expect("switch current generation");
    }
}

fn rollback_bin_dir(paths: &InstallPaths, pending: &PendingRelease) -> PathBuf {
    paths
        .installed_rollback_root()
        .expect("rollback root")
        .join(&pending.generation)
        .join("bin")
}
