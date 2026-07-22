use std::time::Duration;

use chrono::Utc;
use unixnotis_core::Config;

use crate::expire::{ExpirationCommand, ExpirationScheduler};
use crate::store::NotificationStore;
use crate::test_support::{daemon_state_for_test, TempRoot};

#[tokio::test]
async fn dnd_state_rolls_back_when_persistence_fails() {
    let state = daemon_state_for_test(false).await;
    let root = TempRoot::new("dnd-persist-failure");
    let state_dir = root.join("state");
    std::fs::create_dir_all(&state_dir).expect("create state dir");
    std::fs::write(state_dir.join("unixnotis"), "not a directory").expect("block DND parent");
    {
        let mut store = state.store.lock().await;
        *store = NotificationStore::new_with_state_dir(Config::default(), state_dir);
    }

    let error = state
        .apply_dnd_state(true)
        .await
        .expect_err("persistence failure should be reported");

    assert!(error.to_string().contains("failed to persist"));
    assert!(!state.store.lock().await.dnd_enabled());
}

#[tokio::test]
async fn toggled_dnd_persists_the_successful_state_change() {
    let state = daemon_state_for_test(false).await;
    let root = TempRoot::new("dnd-toggle-success");
    let state_dir = root.join("state");
    {
        let mut store = state.store.lock().await;
        *store = NotificationStore::new_with_state_dir(Config::default(), state_dir.clone());
    }

    state
        .apply_toggle_dnd()
        .await
        .expect("toggle should persist");

    assert!(state.store.lock().await.dnd_enabled());
    let persisted = std::fs::read_to_string(state_dir.join("unixnotis").join("state.json"))
        .expect("read persisted DND state");
    assert!(persisted.contains("\"dnd_enabled\":true"));
}

#[tokio::test]
async fn timed_dnd_validates_and_persists_a_future_deadline() {
    let state = daemon_state_for_test(false).await;
    let root = TempRoot::new("dnd-timed-success");
    let state_dir = root.join("state");
    {
        let mut store = state.store.lock().await;
        *store = NotificationStore::new_with_state_dir(Config::default(), state_dir.clone());
    }
    let expires_at = Utc::now().timestamp() + 3_600;

    state
        .apply_dnd_until(expires_at)
        .await
        .expect("timed DND should persist");

    let store = state.store.lock().await;
    assert!(store.dnd_enabled());
    assert_eq!(store.dnd_expires_at(), Some(expires_at));
    drop(store);
    let persisted = std::fs::read_to_string(state_dir.join("unixnotis").join("state.json"))
        .expect("read persisted timed DND state");
    assert!(persisted.contains(&format!("\"expires_at\":{expires_at}")));
}

#[tokio::test]
async fn timed_dnd_rejects_past_and_excessive_deadlines_without_mutation() {
    let state = daemon_state_for_test(false).await;
    let now = Utc::now().timestamp();

    assert!(state.apply_dnd_until(now - 1).await.is_err());
    assert!(state
        .apply_dnd_until(now + 367 * 24 * 60 * 60)
        .await
        .is_err());

    let store = state.store.lock().await;
    assert!(!store.dnd_enabled());
    assert_eq!(store.dnd_expires_at(), None);
}

#[tokio::test]
async fn dnd_updates_wait_for_the_prior_persistence_commit() {
    let state = daemon_state_for_test(false).await;
    let guard = state.lock_dnd_write().await;
    let mut update = Box::pin(state.apply_dnd_state(true));

    assert!(
        tokio::time::timeout(Duration::from_millis(25), &mut update)
            .await
            .is_err(),
        "later DND update should wait for the current writer"
    );
    assert!(!state.store.lock().await.dnd_enabled());

    drop(guard);
    tokio::time::timeout(Duration::from_millis(500), update)
        .await
        .expect("DND update should resume after the prior commit")
        .expect("DND update should succeed");
    assert!(state.store.lock().await.dnd_enabled());
}

#[tokio::test]
async fn cancel_expiration_sends_cancel_command_when_scheduler_is_installed() {
    let state = daemon_state_for_test(false).await;
    let (scheduler, mut receiver) = ExpirationScheduler::channel_for_test();

    state.set_scheduler(scheduler);
    state.cancel_expiration(42);

    let command = tokio::time::timeout(Duration::from_millis(100), receiver.recv())
        .await
        .expect("cancel command should arrive")
        .expect("scheduler channel should stay open");
    match command {
        ExpirationCommand::Cancel { id } => assert_eq!(id, 42),
        ExpirationCommand::Schedule { .. } => panic!("cancel should not schedule a deadline"),
    }
}

#[tokio::test]
async fn cancel_expirations_sends_cancel_for_each_id_in_order() {
    let state = daemon_state_for_test(false).await;
    let (scheduler, mut receiver) = ExpirationScheduler::channel_for_test();

    state.set_scheduler(scheduler);
    state.cancel_expirations(&[7, 8, 9]);

    let mut ids = Vec::new();
    for _ in 0..3 {
        let command = tokio::time::timeout(Duration::from_millis(100), receiver.recv())
            .await
            .expect("cancel command should arrive")
            .expect("scheduler channel should stay open");
        match command {
            ExpirationCommand::Cancel { id } => ids.push(id),
            ExpirationCommand::Schedule { .. } => panic!("cancel should not schedule a deadline"),
        }
    }

    assert_eq!(ids, [7, 8, 9]);
    assert!(receiver.try_recv().is_err());
}

#[tokio::test]
async fn duplicate_scheduler_install_keeps_original_sender() {
    let state = daemon_state_for_test(false).await;
    let (first_scheduler, mut first_receiver) = ExpirationScheduler::channel_for_test();
    let (second_scheduler, mut second_receiver) = ExpirationScheduler::channel_for_test();

    state.set_scheduler(first_scheduler);
    state.set_scheduler(second_scheduler);
    state.cancel_expiration(11);

    let command = tokio::time::timeout(Duration::from_millis(100), first_receiver.recv())
        .await
        .expect("original scheduler should receive cancel")
        .expect("original scheduler channel should stay open");
    match command {
        ExpirationCommand::Cancel { id } => assert_eq!(id, 11),
        ExpirationCommand::Schedule { .. } => panic!("cancel should not schedule a deadline"),
    }
    assert!(second_receiver.try_recv().is_err());
}

#[tokio::test]
async fn missing_scheduler_cancel_is_a_noop() {
    let state = daemon_state_for_test(false).await;

    state.cancel_expiration(1);
    state.cancel_expirations(&[2, 3]);

    assert!(!state.mark_missing_scheduler_warning_needed());
}

#[tokio::test]
async fn missing_scheduler_warning_guard_reports_only_first_missing_scheduler_use() {
    let state = daemon_state_for_test(false).await;

    assert!(state.mark_missing_scheduler_warning_needed());
    assert!(!state.mark_missing_scheduler_warning_needed());
}
