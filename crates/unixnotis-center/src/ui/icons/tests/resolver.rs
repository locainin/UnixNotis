use super::ICON_UPDATE_QUEUE_CAPACITY;

#[test]
fn icon_update_queue_capacity_remains_bounded() {
    assert_eq!(ICON_UPDATE_QUEUE_CAPACITY, 256);
}
