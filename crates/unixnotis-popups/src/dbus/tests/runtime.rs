use super::{build_runtime, UI_COMMAND_QUEUE_CAPACITY};

#[test]
fn popup_runtime_builds_with_a_bounded_command_queue() {
    assert!(build_runtime().is_some());
    assert_eq!(UI_COMMAND_QUEUE_CAPACITY, 64);
}
