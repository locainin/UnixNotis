use super::{clamp_label_text, has_visible_text};

#[test]
fn empty_text_has_no_visible_popup_content() {
    assert!(!has_visible_text(""));
}

#[test]
fn whitespace_only_text_has_no_visible_popup_content() {
    assert!(!has_visible_text("\n\t "));
}

#[test]
fn zero_length_limit_returns_empty_text() {
    assert!(clamp_label_text("hello", 0).is_empty());
}

#[test]
fn nonempty_text_is_visible_even_with_surrounding_whitespace() {
    assert!(has_visible_text("  hello  "));
}

#[test]
fn clamp_preserves_utf8_boundaries_and_adds_ellipsis() {
    assert_eq!(clamp_label_text("éclair", 2).as_ref(), "éc…");
}
