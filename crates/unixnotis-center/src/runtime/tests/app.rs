use super::{build_runtime, UI_EVENT_QUEUE_CAPACITY};

#[test]
fn center_runtime_builds_with_a_bounded_event_queue() {
    assert!(build_runtime().is_some());
    assert_eq!(UI_EVENT_QUEUE_CAPACITY, 512);
}
