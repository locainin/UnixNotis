use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use super::super::manifest::build_manifest;
use super::super::transaction::{
    commit_pending_release, pending_release_exists, rollback_pending_release,
    stage_release_with_copy,
};
use super::{install_release_generation, install_release_generation_with_reservation_check};
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

#[test]
fn successful_install_switches_every_entrypoint_to_one_generation() {
    let root = crate::test_support::fs::unique_temp_path("release-transaction-success");
    let source = root.join("source");
    std::fs::create_dir_all(&source).expect("create binary source");
    let binaries = ["unixnotis-daemon", "unixnotis-center"]
        .into_iter()
        .map(str::to_string)
        .collect::<Vec<_>>();
    for binary in &binaries {
        write_test_binary(&source.join(binary), &format!("payload:{binary}"));
    }
    let paths = paths(&root);

    let generation = install_release_generation(&paths, &source, &binaries, || Ok(()), || Ok(()))
        .expect("install release generation");

    let current = std::fs::read_link(paths.installed_current_link().expect("current path"))
        .expect("current release link");
    assert_eq!(current, Path::new("releases").join(&generation));
    for binary in &binaries {
        assert_eq!(
            std::fs::read_to_string(paths.bin_dir.join(binary)).expect("read linked binary"),
            format!("payload:{binary}")
        );
    }
    assert!(commit_pending_release(&paths).expect("commit pending release"));
    std::fs::remove_dir_all(root).expect("remove release transaction fixture");
}

#[test]
fn failed_precommit_restores_all_legacy_entrypoints() {
    let root = crate::test_support::fs::unique_temp_path("release-transaction-precommit");
    let source = root.join("source");
    let paths = paths(&root);
    std::fs::create_dir_all(&source).expect("create binary source");
    std::fs::create_dir_all(&paths.bin_dir).expect("create legacy binary directory");
    let binaries = ["unixnotis-daemon", "unixnotis-center"]
        .into_iter()
        .map(str::to_string)
        .collect::<Vec<_>>();
    for binary in &binaries {
        write_test_binary(&source.join(binary), &format!("new:{binary}"));
        std::fs::write(paths.bin_dir.join(binary), format!("old:{binary}"))
            .expect("write legacy binary");
    }

    let checks = std::cell::Cell::new(0usize);
    let error = install_release_generation_with_reservation_check(
        &paths,
        &source,
        &binaries,
        || {
            let check = checks.get();
            checks.set(check.saturating_add(1));
            if check == 0 {
                Ok(())
            } else {
                Err(anyhow::anyhow!("daemon restarted"))
            }
        },
        || Ok(()),
        || {
            let check = checks.get();
            checks.set(check.saturating_add(1));
            if check == 1 {
                Err(anyhow::anyhow!("daemon restarted"))
            } else {
                Ok(())
            }
        },
    )
    .expect_err("failed precommit must roll back");

    assert!(format!("{error:#}").contains("daemon restarted"));
    assert_eq!(checks.get(), 2);
    for binary in &binaries {
        assert_eq!(
            std::fs::read_to_string(paths.bin_dir.join(binary)).expect("read restored binary"),
            format!("old:{binary}")
        );
    }
    assert!(!rollback_pending_release(&paths).expect("no pending rollback should remain"));
    std::fs::remove_dir_all(root).expect("remove release rollback fixture");
}

#[test]
fn durable_journal_precedes_the_first_live_entrypoint_mutation() {
    let root = crate::test_support::fs::unique_temp_path("release-journal-order");
    let source = root.join("source");
    let paths = paths(&root);
    std::fs::create_dir_all(&source).expect("create binary source");
    std::fs::create_dir_all(&paths.bin_dir).expect("create legacy binary directory");
    let binary = "unixnotis-daemon".to_string();
    write_test_binary(&source.join(&binary), "new generation");
    std::fs::write(paths.bin_dir.join(&binary), "legacy generation").expect("write legacy binary");
    let checks = std::cell::Cell::new(0usize);

    install_release_generation_with_reservation_check(
        &paths,
        &source,
        std::slice::from_ref(&binary),
        || {
            let check = checks.get();
            checks.set(check.saturating_add(1));
            let metadata = std::fs::symlink_metadata(paths.bin_dir.join(&binary))
                .expect("inspect live entrypoint at precommit");
            match check {
                0 => {
                    assert!(!pending_release_exists(&paths).expect("inspect initial journal"));
                    assert!(metadata.file_type().is_file());
                }
                _ => panic!("unexpected precommit check {check}"),
            }
            Ok(())
        },
        || Ok(()),
        || {
            let check = checks.get();
            checks.set(check.saturating_add(1));
            let metadata = std::fs::symlink_metadata(paths.bin_dir.join(&binary))
                .expect("inspect live entrypoint at reserved check");
            match check {
                1 => {
                    assert!(!pending_release_exists(&paths).expect("inspect initial journal"));
                    assert!(metadata.file_type().is_file());
                }
                2 => {
                    assert!(!pending_release_exists(&paths).expect("inspect pre-recovery journal"));
                    assert!(
                        metadata.file_type().is_file(),
                        "service check must run before pending recovery"
                    );
                }
                3 => {
                    assert!(pending_release_exists(&paths).expect("inspect durable journal"));
                    assert!(
                        metadata.file_type().is_file(),
                        "journal must be durable before the legacy entrypoint moves"
                    );
                }
                4 => {
                    assert!(pending_release_exists(&paths).expect("inspect activation journal"));
                    assert!(
                        metadata.file_type().is_symlink(),
                        "entrypoint publication must finish before the generation switch check"
                    );
                }
                _ => panic!("unexpected reserved check {check}"),
            }
            Ok(())
        },
    )
    .expect("install journal ordering generation");

    assert_eq!(checks.get(), 5);
    assert!(commit_pending_release(&paths).expect("commit journal ordering generation"));
    std::fs::remove_dir_all(root).expect("remove journal ordering fixture");
}

#[test]
fn activation_reservation_is_held_before_layout_and_until_current_switches() {
    struct SwitchObserver {
        current: std::path::PathBuf,
        observed_switch: std::rc::Rc<std::cell::Cell<bool>>,
    }

    impl Drop for SwitchObserver {
        fn drop(&mut self) {
            self.observed_switch
                .set(std::fs::read_link(&self.current).is_ok());
        }
    }

    let root = crate::test_support::fs::unique_temp_path("release-reservation-order");
    let source = root.join("source");
    let paths = paths(&root);
    std::fs::create_dir_all(&source).expect("create binary source");
    let binary = "unixnotis-daemon".to_string();
    write_test_binary(&source.join(&binary), "reserved generation");
    std::fs::create_dir_all(&paths.bin_dir).expect("create legacy entrypoint directory");
    std::fs::write(paths.bin_dir.join(&binary), "legacy generation")
        .expect("write legacy entrypoint");
    let checks = std::cell::Cell::new(0usize);
    let reservation_calls = std::cell::Cell::new(0usize);
    let observed_switch = std::rc::Rc::new(std::cell::Cell::new(false));

    install_release_generation_with_reservation_check(
        &paths,
        &source,
        std::slice::from_ref(&binary),
        || {
            checks.set(checks.get().saturating_add(1));
            Ok(())
        },
        || {
            assert_eq!(
                checks.get(),
                1,
                "reservation must follow the initial unowned-state check"
            );
            assert!(
                std::fs::symlink_metadata(paths.bin_dir.join(&binary))
                    .expect("inspect entrypoint before reservation")
                    .file_type()
                    .is_file(),
                "activation reservation must precede entrypoint mutation"
            );
            assert!(
                !pending_release_exists(&paths).expect("inspect journal before reservation"),
                "activation reservation must precede journal publication"
            );
            reservation_calls.set(reservation_calls.get().saturating_add(1));
            Ok(SwitchObserver {
                current: paths.installed_current_link().expect("current link path"),
                observed_switch: std::rc::Rc::clone(&observed_switch),
            })
        },
        || Ok(()),
    )
    .expect("activate generation under reservation");

    assert_eq!(reservation_calls.get(), 1);
    assert!(
        observed_switch.get(),
        "activation reservation must remain alive through the current-link switch"
    );
    assert!(commit_pending_release(&paths).expect("commit reserved generation"));
    std::fs::remove_dir_all(root).expect("remove reservation ordering fixture");
}

#[test]
fn failure_copying_any_binary_publishes_no_partial_generation() {
    for failed_index in 0..3 {
        let root = crate::test_support::fs::unique_temp_path(&format!(
            "release-copy-failure-{failed_index}"
        ));
        let source = root.join("source");
        std::fs::create_dir_all(&source).expect("create binary source");
        let sources = ["first", "middle", "final"]
            .into_iter()
            .map(|name| {
                let path = source.join(name);
                write_test_binary(&path, name);
                (name.to_string(), path)
            })
            .collect::<Vec<_>>();
        let manifest = build_manifest(&sources).expect("build test manifest");
        let paths = paths(&root);
        let mut copied = 0usize;

        let error = stage_release_with_copy(
            &paths,
            "copy-failure",
            &sources,
            &manifest,
            |source, destination| {
                if copied == failed_index {
                    return Err(anyhow::anyhow!("injected copy failure"));
                }
                copied = copied.saturating_add(1);
                unixnotis_core::filesystem::copy_file_atomic(source, destination)
                    .map_err(anyhow::Error::from)
            },
        )
        .expect_err("injected copy failure must abort staging");

        assert!(error.to_string().contains("stage release binary"));
        let entries = std::fs::read_dir(paths.installed_releases_dir().expect("releases path"))
            .expect("read releases directory")
            .collect::<Result<Vec<_>, _>>()
            .expect("collect releases directory");
        assert!(
            entries.is_empty(),
            "copy failure {failed_index} published partial release state"
        );
        std::fs::remove_dir_all(root).expect("remove copy failure fixture");
    }
}

#[test]
fn readiness_rollback_restores_the_complete_previous_generation() {
    let root = crate::test_support::fs::unique_temp_path("release-readiness-rollback");
    let source = root.join("source");
    std::fs::create_dir_all(&source).expect("create binary source");
    let binaries = ["unixnotis-daemon", "unixnotis-center"]
        .into_iter()
        .map(str::to_string)
        .collect::<Vec<_>>();
    for binary in &binaries {
        write_test_binary(&source.join(binary), &format!("old:{binary}"));
    }
    let paths = paths(&root);
    let old_generation =
        install_release_generation(&paths, &source, &binaries, || Ok(()), || Ok(()))
            .expect("install old generation");
    commit_pending_release(&paths).expect("commit old generation");
    for binary in &binaries {
        write_test_binary(&source.join(binary), &format!("new:{binary}"));
    }
    install_release_generation(&paths, &source, &binaries, || Ok(()), || Ok(()))
        .expect("activate pending new generation");

    assert!(rollback_pending_release(&paths).expect("roll back failed readiness"));

    assert_eq!(
        std::fs::read_link(paths.installed_current_link().expect("current path"))
            .expect("restored current release"),
        Path::new("releases").join(old_generation)
    );
    for binary in &binaries {
        assert_eq!(
            std::fs::read_to_string(paths.bin_dir.join(binary)).expect("read restored binary"),
            format!("old:{binary}")
        );
    }
    std::fs::remove_dir_all(root).expect("remove readiness rollback fixture");
}

#[test]
fn recovery_finishes_when_current_was_restored_before_a_crash() {
    let root = crate::test_support::fs::unique_temp_path("release-idempotent-current-rollback");
    let source = root.join("source");
    std::fs::create_dir_all(&source).expect("create idempotent rollback source");
    let paths = paths(&root);
    let binary = "unixnotis-daemon".to_string();
    write_test_binary(&source.join(&binary), "old generation");
    let old_generation = install_release_generation(
        &paths,
        &source,
        std::slice::from_ref(&binary),
        || Ok(()),
        || Ok(()),
    )
    .expect("install previous generation");
    commit_pending_release(&paths).expect("commit previous generation");
    write_test_binary(&source.join(&binary), "new generation");
    install_release_generation(
        &paths,
        &source,
        std::slice::from_ref(&binary),
        || Ok(()),
        || Ok(()),
    )
    .expect("activate new generation");
    let previous_target = Path::new("releases").join(&old_generation);
    unixnotis_core::filesystem::replace_symlink_atomic(
        &paths.installed_current_link().expect("current path"),
        &previous_target,
    )
    .expect("simulate completed current-link rollback");

    assert!(rollback_pending_release(&paths).expect("finish idempotent rollback"));

    assert_eq!(
        std::fs::read_link(paths.installed_current_link().expect("current path"))
            .expect("retained previous generation"),
        previous_target
    );
    assert!(!pending_release_exists(&paths).expect("journal removed after recovery"));
    std::fs::remove_dir_all(root).expect("remove idempotent rollback fixture");
}

#[test]
fn readiness_rollback_refuses_a_previous_generation_that_changed_after_activation() {
    let root = crate::test_support::fs::unique_temp_path("release-rollback-revalidation");
    let source = root.join("source");
    std::fs::create_dir_all(&source).expect("create rollback revalidation source");
    let paths = paths(&root);
    let binary = "unixnotis-daemon".to_string();
    write_test_binary(&source.join(&binary), "old generation");
    let old_generation = install_release_generation(
        &paths,
        &source,
        std::slice::from_ref(&binary),
        || Ok(()),
        || Ok(()),
    )
    .expect("install previous generation");
    commit_pending_release(&paths).expect("commit previous generation");
    write_test_binary(&source.join(&binary), "new generation");
    let new_generation = install_release_generation(
        &paths,
        &source,
        std::slice::from_ref(&binary),
        || Ok(()),
        || Ok(()),
    )
    .expect("activate new generation");
    let old_binary = paths
        .installed_releases_dir()
        .expect("installed releases directory")
        .join(old_generation)
        .join("bin")
        .join(&binary);
    std::fs::write(old_binary, "corrupted old!").expect("corrupt previous generation");

    let error = rollback_pending_release(&paths)
        .expect_err("changed previous generation must not be reactivated");

    assert!(error
        .to_string()
        .contains("verify previous release generation before rollback"));
    assert_eq!(
        std::fs::read_link(paths.installed_current_link().expect("current path"))
            .expect("retain current generation"),
        Path::new("releases").join(new_generation)
    );
    assert!(pending_release_exists(&paths).expect("retain pending rollback journal"));
    std::fs::remove_dir_all(root).expect("remove rollback revalidation fixture");
}

#[test]
fn successful_commits_retain_only_current_and_previous_verified_generations() {
    let root = crate::test_support::fs::unique_temp_path("release-retention");
    let source = root.join("source");
    std::fs::create_dir_all(&source).expect("create retention source");
    let paths = paths(&root);
    let binary = "unixnotis-daemon".to_string();
    let mut generations = Vec::new();

    for payload in ["generation one", "generation two", "generation three"] {
        write_test_binary(&source.join(&binary), payload);
        let generation = install_release_generation(
            &paths,
            &source,
            std::slice::from_ref(&binary),
            || Ok(()),
            || Ok(()),
        )
        .expect("install retention generation");
        assert!(commit_pending_release(&paths).expect("commit retention generation"));
        generations.push(generation);
    }

    let releases = paths
        .installed_releases_dir()
        .expect("installed releases directory");
    assert!(!releases.join(&generations[0]).exists());
    assert!(releases.join(&generations[1]).exists());
    assert!(releases.join(&generations[2]).exists());
    std::fs::remove_dir_all(root).expect("remove retention fixture");
}

#[test]
fn readiness_commit_revalidates_the_generation_before_discarding_rollback() {
    let root = crate::test_support::fs::unique_temp_path("release-commit-revalidation");
    let source = root.join("source");
    std::fs::create_dir_all(&source).expect("create revalidation source");
    let paths = paths(&root);
    let binary = "unixnotis-daemon".to_string();
    write_test_binary(&source.join(&binary), "generation payload");
    let generation = install_release_generation(
        &paths,
        &source,
        std::slice::from_ref(&binary),
        || Ok(()),
        || Ok(()),
    )
    .expect("install pending generation");
    let installed_binary = paths
        .installed_releases_dir()
        .expect("installed releases directory")
        .join(generation)
        .join("bin")
        .join(&binary);
    std::fs::write(&installed_binary, "changed payload!!")
        .expect("corrupt pending generation after activation");

    assert!(commit_pending_release(&paths).is_err());
    assert!(rollback_pending_release(&paths).expect("pending rollback remains recoverable"));
    std::fs::remove_dir_all(root).expect("remove revalidation fixture");
}

fn write_test_binary(path: &Path, contents: &str) {
    std::fs::write(path, contents).expect("write release test binary");
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755))
        .expect("make release test binary executable");
}
