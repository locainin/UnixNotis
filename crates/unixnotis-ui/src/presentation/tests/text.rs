use super::super::{clamp_label_text, has_visible_text};

#[test]
fn blank_text_has_no_visible_notification_content() {
    assert!(!has_visible_text(""));
    assert!(!has_visible_text("\n\t "));
}

#[test]
fn nonempty_text_remains_visible_with_surrounding_whitespace() {
    assert!(has_visible_text("  hello  "));
}

#[test]
fn shared_clamp_preserves_utf8_and_zero_limit_semantics() {
    assert!(clamp_label_text("hello", 0).is_empty());
    assert_eq!(clamp_label_text("éclair", 2).as_ref(), "éc…");
}
