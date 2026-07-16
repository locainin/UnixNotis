use super::autoclose::should_connect_blur_close;

#[test]
fn blur_close_is_used_when_outside_watcher_is_unavailable() {
    assert!(should_connect_blur_close(true, true, false));
    assert!(!should_connect_blur_close(true, true, true));
}

#[test]
fn blur_close_respects_its_own_setting_without_outside_tracking() {
    assert!(should_connect_blur_close(false, true, false));
    assert!(!should_connect_blur_close(false, false, false));
}
