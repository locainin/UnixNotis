use crate::test_support::fs::write_executable;

use super::super::test_support::{fake_daemon_tool_root, test_install_paths};
use super::{
    ensure_selected_service_inactive_until, wait_until_no_conflicting_live_daemon,
    wait_until_no_conflicting_live_daemon_with_probe,
};

fn one_shot_live_daemon_check(paths: &crate::paths::InstallPaths) -> anyhow::Result<()> {
    let deadline = std::time::Instant::now()
        .checked_add(crate::service_manager::contract::ServiceProbe::default_timeout())
        .ok_or_else(|| anyhow::anyhow!("daemon check deadline exceeded the monotonic clock"))?;
    super::ensure_no_conflicting_live_daemon_until(paths, deadline)
}

#[test]
fn daemon_quiescence_wait_retries_until_broker_and_manager_are_inactive() {
    let attempts = std::cell::Cell::new(0usize);

    wait_until_no_conflicting_live_daemon_with_probe(
        std::time::Duration::from_secs(1),
        std::time::Duration::ZERO,
        |_deadline| {
            let attempt = attempts.get();
            attempts.set(attempt.saturating_add(1));
            if attempt < 2 {
                Err(anyhow::anyhow!("runtime still live"))
            } else {
                Ok(())
            }
        },
    )
    .expect("runtime should become quiescent after bounded retries");

    assert_eq!(attempts.get(), 3);
}

#[test]
fn daemon_quiescence_wait_preserves_indeterminate_state_at_timeout() {
    let attempts = std::cell::Cell::new(0usize);

    let error = wait_until_no_conflicting_live_daemon_with_probe(
        std::time::Duration::ZERO,
        std::time::Duration::ZERO,
        |_deadline| {
            attempts.set(attempts.get().saturating_add(1));
            Err(anyhow::anyhow!("broker inspection failed"))
        },
    )
    .expect_err("indeterminate runtime state must fail closed at the deadline");

    assert_eq!(attempts.get(), 1);
    assert!(error
        .to_string()
        .contains("notification runtime did not become quiescent"));
}

#[test]
fn production_quiescence_wait_rejects_an_elapsed_deadline() {
    let paths = test_install_paths();

    wait_until_no_conflicting_live_daemon(&paths, std::time::Duration::ZERO)
        .expect_err("an elapsed production deadline must fail closed");
}

#[test]
fn selected_service_recheck_rejects_an_active_manager() {
    let root = fake_daemon_tool_root("active-selected-service");
    write_executable(
        &root.join("systemctl"),
        "#!/bin/sh\nprintf 'LoadState=loaded\\nActiveState=active\\n'\n",
    );
    let _commands = crate::service_manager::contract::command_routing::use_fake_command_bin(&root);
    let paths = test_install_paths();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(1);

    let error = ensure_selected_service_inactive_until(&paths, deadline)
        .expect_err("an active selected service must block activation");

    assert!(error.to_string().contains("became active again"));
    std::fs::remove_dir_all(root).expect("remove active service fixture");
}

#[test]
fn selected_service_recheck_rejects_an_operational_probe_failure() {
    let root = fake_daemon_tool_root("indeterminate-selected-service");
    write_executable(
        &root.join("systemctl"),
        "#!/bin/sh\nprintf 'Failed to connect to bus\\n' >&2\nexit 1\n",
    );
    let _commands = crate::service_manager::contract::command_routing::use_fake_command_bin(&root);
    let paths = test_install_paths();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(1);

    let error = ensure_selected_service_inactive_until(&paths, deadline)
        .expect_err("an indeterminate selected service state must block activation");

    assert!(error.to_string().contains("indeterminate state"));
    std::fs::remove_dir_all(root).expect("remove indeterminate service fixture");
}

#[test]
fn selected_service_recheck_accepts_an_absent_unit() {
    let root = fake_daemon_tool_root("absent-selected-service");
    write_executable(
        &root.join("systemctl"),
        "#!/bin/sh\nprintf 'LoadState=not-found\nActiveState=inactive\n'\n",
    );
    let _commands = crate::service_manager::contract::command_routing::use_fake_command_bin(&root);
    let paths = test_install_paths();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(1);

    ensure_selected_service_inactive_until(&paths, deadline)
        .expect("an absent selected service is safe before first binary activation");

    std::fs::remove_dir_all(root).expect("remove absent service fixture");
}

#[test]
fn generation_precommit_rejects_a_daemon_that_reappeared_after_stop() {
    let root = fake_daemon_tool_root("fresh-owner-before-commit");
    write_executable(
        &root.join("busctl"),
        "#!/bin/sh\ncase \"$*\" in *NameHasOwner*) printf 'b true\\n' ;; *GetNameOwner*) printf 's \":1.100\"\\n' ;; *'status :1.100'*) printf 'Comm=unixnotis-daemon\\n' ;; *) exit 1 ;; esac\n",
    );
    let _tools = crate::system_tools::routing::use_fake_tool_bin(&root);
    let paths = test_install_paths();

    let error = one_shot_live_daemon_check(&paths)
        .expect_err("a daemon appearing before activation must block the generation switch");

    assert!(
        error
            .to_string()
            .contains("notification daemon appeared before binary activation"),
        "unexpected precommit owner error: {error:#}"
    );
    std::fs::remove_dir_all(root).expect("remove precommit owner fixture");
}
