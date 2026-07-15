use super::VisiblePopupUpdate;

#[test]
fn visible_update_starts_without_stack_changes() {
    let update = VisiblePopupUpdate::default();

    assert!(!update.stack_changed);
}
