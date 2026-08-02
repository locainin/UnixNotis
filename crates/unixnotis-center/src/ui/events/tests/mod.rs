use super::should_snap_to_top_value;

#[test]
fn near_top_insertions_snap_to_the_first_row() {
    assert!(should_snap_to_top_value(0.0, 0.0));
    assert!(should_snap_to_top_value(17.5, 0.0));
    assert!(!should_snap_to_top_value(18.1, 0.0));
}

#[test]
fn scroll_threshold_follows_nonzero_adjustment_lower_bound() {
    assert!(should_snap_to_top_value(118.0, 100.0));
    assert!(!should_snap_to_top_value(118.1, 100.0));
}
