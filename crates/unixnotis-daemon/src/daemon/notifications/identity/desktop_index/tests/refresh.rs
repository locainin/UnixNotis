use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use notify::event::{CreateKind, RemoveKind};
use notify::{Event, EventKind};

use std::time::Duration;

use super::{
    fallback_required, has_incomplete_watch_coverage, install_healthy_replacement,
    queue_refresh_event, rebuild_delay, registration_is_complete, relevant_desktop_event,
    DesktopIndexRefreshHandle, RefreshTrigger, WatcherHealth, WatcherInstance,
};
use crate::test_support::TempRoot;

#[test]
fn desktop_file_changes_request_an_index_refresh() {
    let event = Event::new(EventKind::Any).add_path("org.example.App.desktop".into());

    assert!(relevant_desktop_event(&event));
}

#[test]
fn unrelated_regular_file_changes_do_not_request_an_index_refresh() {
    let event = Event::new(EventKind::Any).add_path("notes.txt".into());

    assert!(!relevant_desktop_event(&event));
}

#[test]
fn relevant_event_is_queued_for_the_async_refresh_loop() {
    let (refresh_tx, mut refresh_rx) = tokio::sync::mpsc::channel(1);
    let health = WatcherHealth::default();
    health.set_installed(true);
    let event = Event::new(EventKind::Any).add_path("org.example.App.desktop".into());

    queue_refresh_event(Ok(event), &refresh_tx, &health);

    assert_eq!(refresh_rx.try_recv(), Ok(RefreshTrigger::Filesystem));
}

#[test]
fn unrelated_event_is_not_queued_for_the_async_refresh_loop() {
    let (refresh_tx, mut refresh_rx) = tokio::sync::mpsc::channel(1);
    let health = WatcherHealth::default();
    health.set_installed(true);
    let event = Event::new(EventKind::Any).add_path("notes.txt".into());

    queue_refresh_event(Ok(event), &refresh_tx, &health);

    assert_eq!(
        refresh_rx.try_recv(),
        Err(tokio::sync::mpsc::error::TryRecvError::Empty)
    );
}

#[test]
fn watcher_errors_request_fallback_refreshes() {
    let (refresh_tx, mut refresh_rx) = tokio::sync::mpsc::channel(1);
    let health = WatcherHealth::default();
    health.set_installed(true);

    queue_refresh_event(
        Err(notify::Error::generic("watcher failure")),
        &refresh_tx,
        &health,
    );

    assert_eq!(refresh_rx.try_recv(), Ok(RefreshTrigger::WatchError));
    assert!(health.is_degraded());
}

#[test]
fn watcher_error_is_retained_when_trigger_channel_is_full() {
    let (refresh_tx, mut refresh_rx) = tokio::sync::mpsc::channel(1);
    let health = WatcherHealth::default();
    health.set_installed(true);
    let filesystem_event = Event::new(EventKind::Any).add_path("org.example.App.desktop".into());

    queue_refresh_event(Ok(filesystem_event), &refresh_tx, &health);
    queue_refresh_event(
        Err(notify::Error::generic("watcher failure")),
        &refresh_tx,
        &health,
    );

    assert_eq!(refresh_rx.try_recv(), Ok(RefreshTrigger::Filesystem));
    assert_eq!(
        refresh_rx.try_recv(),
        Err(tokio::sync::mpsc::error::TryRecvError::Empty)
    );
    assert!(health.is_degraded());
}

#[test]
fn degraded_watcher_keeps_fallback_after_rebuild_with_full_coverage() {
    assert!(fallback_required(true, false));
    assert!(fallback_required(false, true));
    assert!(!fallback_required(false, false));
}

#[test]
fn manual_refresh_is_enqueued_without_running_a_second_worker() {
    let (refresh_tx, mut refresh_rx) = tokio::sync::mpsc::channel(1);
    let handle = DesktopIndexRefreshHandle { refresh_tx };

    assert!(handle.request_manual());
    assert_eq!(refresh_rx.try_recv(), Ok(RefreshTrigger::Manual));
}

#[test]
fn existing_directory_changes_request_watch_set_refresh() {
    let root = TempRoot::new("desktop-refresh-directory");
    let directory = root.join("nested");
    fs::create_dir(&directory).expect("create watched directory fixture");
    let event = Event::new(EventKind::Any).add_path(directory);

    assert!(relevant_desktop_event(&event));
}

#[test]
fn removed_directory_events_request_watch_set_refresh() {
    let event =
        Event::new(EventKind::Remove(RemoveKind::Folder)).add_path("removed-directory".into());

    assert!(relevant_desktop_event(&event));
}

#[test]
fn created_directory_events_request_watch_set_refresh() {
    let event = Event::new(EventKind::Create(CreateKind::Folder)).add_path("new-directory".into());

    assert!(relevant_desktop_event(&event));
}

#[test]
fn rebuild_delay_enforces_the_minimum_interval_without_oversleeping() {
    assert_eq!(
        rebuild_delay(Duration::from_secs(2)),
        Duration::from_secs(3)
    );
    assert_eq!(rebuild_delay(Duration::from_secs(5)), Duration::ZERO);
    assert_eq!(rebuild_delay(Duration::from_secs(8)), Duration::ZERO);
}

#[test]
fn incomplete_watch_coverage_requires_periodic_rebuilds() {
    let requested = HashSet::from([PathBuf::from("/apps/a"), PathBuf::from("/apps/b")]);
    let one_active = HashSet::from([PathBuf::from("/apps/a")]);
    let empty = HashSet::new();

    assert!(has_incomplete_watch_coverage(&requested, &one_active));
    assert!(has_incomplete_watch_coverage(&requested, &empty));
    assert!(has_incomplete_watch_coverage(&empty, &empty));
    assert!(!has_incomplete_watch_coverage(&requested, &requested));
}

#[test]
fn equal_watch_counts_do_not_imply_complete_coverage() {
    let requested = HashSet::from([PathBuf::from("/apps/a"), PathBuf::from("/apps/b")]);
    let active = HashSet::from([PathBuf::from("/apps/a"), PathBuf::from("/apps/c")]);

    assert!(has_incomplete_watch_coverage(&requested, &active));
    assert!(!registration_is_complete(&requested, &active));
}

#[test]
fn setup_errors_mark_a_candidate_without_waking_the_worker() {
    let health = WatcherHealth::default();

    assert!(!health.record_error());
    assert!(health.is_degraded());
}

#[test]
fn installed_watcher_error_wakes_the_worker_once() {
    let health = WatcherHealth::default();
    health.set_installed(true);

    assert!(health.record_error());
    assert!(!health.record_error());
    assert!(health.is_degraded());
}

#[test]
fn partial_replacement_is_rejected() {
    let requested = HashSet::from([PathBuf::from("/apps/a"), PathBuf::from("/apps/b")]);
    let old_health = Arc::new(WatcherHealth::default());
    old_health.set_installed(true);
    let mut current = WatcherInstance {
        monitor: (),
        active_watches: requested.clone(),
        health: Arc::clone(&old_health),
    };
    let candidate = WatcherInstance {
        monitor: (),
        active_watches: HashSet::from([PathBuf::from("/apps/a")]),
        health: Arc::new(WatcherHealth::default()),
    };

    assert!(!install_healthy_replacement(
        &mut current,
        candidate,
        &requested
    ));
    assert!(current.health.accepts_events());
    assert!(Arc::ptr_eq(&current.health, &old_health));
}

#[test]
fn degraded_replacement_is_rejected() {
    let requested = HashSet::from([PathBuf::from("/apps/a")]);
    let old_health = Arc::new(WatcherHealth::default());
    old_health.set_installed(true);
    let mut current = WatcherInstance {
        monitor: (),
        active_watches: requested.clone(),
        health: Arc::clone(&old_health),
    };
    let candidate_health = Arc::new(WatcherHealth::default());
    candidate_health.record_error();
    let candidate = WatcherInstance {
        monitor: (),
        active_watches: requested.clone(),
        health: candidate_health,
    };

    assert!(!install_healthy_replacement(
        &mut current,
        candidate,
        &requested
    ));
    assert!(current.health.accepts_events());
    assert!(Arc::ptr_eq(&current.health, &old_health));
}

#[test]
fn healthy_replacement_transfers_event_ownership() {
    let requested = HashSet::from([PathBuf::from("/apps/a")]);
    let old_health = Arc::new(WatcherHealth::default());
    old_health.set_installed(true);
    old_health.record_error();
    let candidate_health = Arc::new(WatcherHealth::default());
    let candidate_health_for_assertion = Arc::clone(&candidate_health);
    let mut current = WatcherInstance {
        monitor: (),
        active_watches: requested.clone(),
        health: Arc::clone(&old_health),
    };
    let candidate = WatcherInstance {
        monitor: (),
        active_watches: requested.clone(),
        health: candidate_health,
    };

    assert!(install_healthy_replacement(
        &mut current,
        candidate,
        &requested
    ));
    assert!(!current.health.is_degraded());
    assert!(current.health.accepts_events());
    assert!(!old_health.accepts_events());
    old_health.record_error();
    assert!(!current.health.is_degraded());
    assert!(Arc::ptr_eq(
        &current.health,
        &candidate_health_for_assertion
    ));
}

#[test]
fn empty_registration_needs_fallback_but_can_be_replaced() {
    let empty = HashSet::new();
    assert!(registration_is_complete(&empty, &empty));
    assert!(has_incomplete_watch_coverage(&empty, &empty));
}

#[test]
fn successful_replacement_queues_recovery_verification() {
    let (tx, mut rx) = tokio::sync::mpsc::channel(1);
    super::queue_recovery_verification(&tx);

    assert_eq!(rx.try_recv(), Ok(RefreshTrigger::RecoveryVerification));
}
