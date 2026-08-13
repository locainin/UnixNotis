use anyhow::anyhow;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use super::super::recovery::{
    recover_guarded_failure_with_hooks, recover_install_failure, InstallFailureRecovery,
};
use super::support::{guarded_lifecycle, recovery_context, recovery_paths};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum WorkerFailureAction {
    Return,
    HoldActivation,
}

const fn worker_failure_action(recovery: &InstallFailureRecovery) -> WorkerFailureAction {
    match recovery {
        InstallFailureRecovery::Recovered(_) => WorkerFailureAction::Return,
        InstallFailureRecovery::ActivationInhibited(_) => WorkerFailureAction::HoldActivation,
    }
}

#[test]
fn pending_journal_inspection_failure_keeps_activation_inhibited() {
    let root = crate::test_support::fs::unique_temp_path("workflow-pending-inspection-failure");
    let paths = recovery_paths(&root);
    let mut ctx = recovery_context(&paths);
    let alive = Arc::new(AtomicBool::new(false));
    let mut lifecycle = guarded_lifecycle(&alive);

    let recovery = recover_guarded_failure_with_hooks(
        &mut ctx,
        &mut lifecycle,
        anyhow!("install failed"),
        Err(anyhow!("journal unreadable")),
        |_ctx, _reservation| Ok(false),
        |_ctx| Ok(()),
    );

    assert!(matches!(
        recovery,
        InstallFailureRecovery::ActivationInhibited(_)
    ));
    assert!(lifecycle.activation.is_some());
    assert!(alive.load(Ordering::Acquire));
    std::fs::remove_dir_all(root).expect("remove recovery fixture");
}

#[test]
fn guarded_rollback_failure_keeps_activation_inhibited() {
    let root = crate::test_support::fs::unique_temp_path("workflow-rollback-failure");
    let paths = recovery_paths(&root);
    let mut ctx = recovery_context(&paths);
    let alive = Arc::new(AtomicBool::new(false));
    let mut lifecycle = guarded_lifecycle(&alive);

    let recovery = recover_guarded_failure_with_hooks(
        &mut ctx,
        &mut lifecycle,
        anyhow!("install failed"),
        Ok(true),
        |_ctx, _reservation| Err(anyhow!("rollback failed")),
        |_ctx| Ok(()),
    );

    assert!(matches!(
        recovery,
        InstallFailureRecovery::ActivationInhibited(_)
    ));
    assert!(lifecycle.activation.is_some());
    assert!(alive.load(Ordering::Acquire));
    std::fs::remove_dir_all(root).expect("remove recovery fixture");
}

#[test]
fn successful_guarded_rollback_releases_before_previous_restart() {
    let root = crate::test_support::fs::unique_temp_path("workflow-rollback-success");
    let paths = recovery_paths(&root);
    let mut ctx = recovery_context(&paths);
    let alive = Arc::new(AtomicBool::new(false));
    let mut lifecycle = guarded_lifecycle(&alive);
    let restart_saw_released = Arc::clone(&alive);

    let recovery = recover_guarded_failure_with_hooks(
        &mut ctx,
        &mut lifecycle,
        anyhow!("install failed"),
        Ok(true),
        |_ctx, _reservation| Ok(true),
        move |_ctx| {
            assert!(!restart_saw_released.load(Ordering::Acquire));
            Ok(())
        },
    );

    assert!(matches!(recovery, InstallFailureRecovery::Recovered(_)));
    assert!(lifecycle.activation.is_none());
    assert!(!alive.load(Ordering::Acquire));
    std::fs::remove_dir_all(root).expect("remove recovery fixture");
}

#[test]
fn missing_pending_journal_without_memory_mutation_is_an_ordinary_failure() {
    let root = crate::test_support::fs::unique_temp_path("workflow-no-pending");
    let paths = recovery_paths(&root);
    let mut ctx = recovery_context(&paths);
    let alive = Arc::new(AtomicBool::new(false));
    let mut lifecycle = guarded_lifecycle(&alive);

    let recovery = recover_guarded_failure_with_hooks(
        &mut ctx,
        &mut lifecycle,
        anyhow!("staging failed"),
        Ok(false),
        |_ctx, _reservation| Ok(false),
        |_ctx| Ok(()),
    );

    assert!(matches!(recovery, InstallFailureRecovery::Recovered(_)));
    assert!(lifecycle.activation.is_none());
    assert!(!alive.load(Ordering::Acquire));
    std::fs::remove_dir_all(root).expect("remove recovery fixture");
}

#[test]
fn missing_pending_journal_with_memory_mutation_is_catastrophic() {
    let root = crate::test_support::fs::unique_temp_path("workflow-pending-contradiction");
    let paths = recovery_paths(&root);
    let mut ctx = recovery_context(&paths);
    let alive = Arc::new(AtomicBool::new(false));
    let mut lifecycle = guarded_lifecycle(&alive);
    lifecycle.release_pending = true;

    let recovery = recover_guarded_failure_with_hooks(
        &mut ctx,
        &mut lifecycle,
        anyhow!("install failed"),
        Ok(false),
        |_ctx, _reservation| Ok(false),
        |_ctx| Ok(()),
    );

    assert!(matches!(
        recovery,
        InstallFailureRecovery::ActivationInhibited(_)
    ));
    assert!(lifecycle.activation.is_some());
    assert!(alive.load(Ordering::Acquire));
    std::fs::remove_dir_all(root).expect("remove recovery fixture");
}

#[test]
fn catastrophic_recovery_selects_hold_action_without_blocking_the_test() {
    let recovered = InstallFailureRecovery::Recovered(anyhow!("ordinary failure"));
    let inhibited = InstallFailureRecovery::ActivationInhibited(anyhow!("unsafe to release"));

    assert_eq!(
        worker_failure_action(&recovered),
        WorkerFailureAction::Return
    );
    assert_eq!(
        worker_failure_action(&inhibited),
        WorkerFailureAction::HoldActivation
    );
}

#[test]
fn worker_recovery_keeps_the_real_guard_when_pending_inspection_fails() {
    let root = crate::test_support::fs::unique_temp_path("workflow-real-pending-error");
    let paths = recovery_paths(&root);
    let pending_path = paths
        .installed_pending_manifest()
        .expect("pending manifest path");
    std::fs::create_dir_all(&pending_path).expect("make unreadable pending manifest object");
    let mut ctx = recovery_context(&paths);
    let alive = Arc::new(AtomicBool::new(false));
    let mut lifecycle = guarded_lifecycle(&alive);

    let recovery = recover_install_failure(&mut ctx, &mut lifecycle, anyhow!("install failed"));

    assert!(matches!(
        recovery,
        InstallFailureRecovery::ActivationInhibited(_)
    ));
    assert!(lifecycle.activation.is_some());
    assert!(alive.load(Ordering::Acquire));
    std::fs::remove_dir_all(root).expect("remove recovery fixture");
}
