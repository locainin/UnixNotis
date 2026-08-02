use std::fs;

use notify::event::{CreateKind, RemoveKind};
use notify::{Event, EventKind};

use std::time::Duration;

use super::{
    has_incomplete_watch_coverage, queue_refresh_event, rebuild_delay, relevant_desktop_event,
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
    let event = Event::new(EventKind::Any).add_path("org.example.App.desktop".into());

    queue_refresh_event(Ok(event), &refresh_tx);

    assert_eq!(refresh_rx.try_recv(), Ok(()));
}

#[test]
fn unrelated_event_is_not_queued_for_the_async_refresh_loop() {
    let (refresh_tx, mut refresh_rx) = tokio::sync::mpsc::channel(1);
    let event = Event::new(EventKind::Any).add_path("notes.txt".into());

    queue_refresh_event(Ok(event), &refresh_tx);

    assert_eq!(
        refresh_rx.try_recv(),
        Err(tokio::sync::mpsc::error::TryRecvError::Empty)
    );
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
    assert!(has_incomplete_watch_coverage(2, 1));
    assert!(has_incomplete_watch_coverage(1, 0));
    assert!(has_incomplete_watch_coverage(0, 0));
    assert!(!has_incomplete_watch_coverage(2, 2));
}
