use super::{generation_matches, incoming_generation_is_stale, VisiblePopupUpdate};

#[test]
fn visible_update_starts_without_stack_changes() {
    let update = VisiblePopupUpdate::default();

    assert!(!update.stack_changed);
}

#[test]
fn newer_popup_generation_rejects_reordered_older_update() {
    assert!(incoming_generation_is_stale(Some(8), 7));
    assert!(!incoming_generation_is_stale(Some(8), 8));
    assert!(!incoming_generation_is_stale(Some(7), 8));
    assert!(!incoming_generation_is_stale(None, 8));
}

#[test]
fn popup_close_matches_only_the_exact_generation() {
    assert!(generation_matches(Some(8), 8));
    assert!(!generation_matches(Some(8), 7));
    assert!(!generation_matches(None, 8));
}
