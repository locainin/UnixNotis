use super::icon_name_is_usable;

#[test]
fn empty_icon_name_is_not_resolved() {
    assert!(!icon_name_is_usable(""));
}

#[test]
fn nonempty_icon_name_is_resolved_without_rewriting() {
    assert!(icon_name_is_usable("application-x-executable"));
}
