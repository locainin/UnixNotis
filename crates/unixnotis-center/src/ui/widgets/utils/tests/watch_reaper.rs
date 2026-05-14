use super::{enqueue_watch_cleanup, reaper_sender};

#[test]
fn watch_reaper_initializes_once() {
    // The shared cleanup sender should stay stable across repeated lookups
    let first = reaper_sender().map(|sender| sender as *const _);
    let second = reaper_sender().map(|sender| sender as *const _);

    assert_eq!(first, second);
}

#[test]
fn watch_cleanup_accepts_empty_jobs() {
    // Empty jobs should still be accepted so callers can keep one code path
    enqueue_watch_cleanup("test".to_string(), None, None);
}
