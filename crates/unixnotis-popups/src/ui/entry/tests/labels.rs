use super::{
    clamp_label_text, optional_label_state, POPUP_BODY_MAX_CHARS, POPUP_SUMMARY_MAX_CHARS,
};

#[test]
fn summary_row_hides_when_text_is_empty() {
    let state = optional_label_state("", POPUP_SUMMARY_MAX_CHARS);

    assert!(!state.visible);
    assert!(state.text.is_empty());
}

#[test]
fn body_row_hides_when_text_is_only_whitespace() {
    let state = optional_label_state("\n\t ", POPUP_BODY_MAX_CHARS);

    assert!(!state.visible);
    assert!(state.text.is_empty());
}

#[test]
fn zero_length_limit_hides_nonempty_text() {
    let state = optional_label_state("hello", 0);

    assert!(!state.visible);
    assert!(state.text.is_empty());
}

#[test]
fn visible_text_preserves_surrounding_whitespace() {
    let state = optional_label_state("  hello  ", POPUP_SUMMARY_MAX_CHARS);

    assert!(state.visible);
    assert_eq!(state.text.as_ref(), "  hello  ");
}

#[test]
fn clamp_preserves_utf8_boundaries_and_adds_ellipsis() {
    assert_eq!(clamp_label_text("éclair", 2).as_ref(), "éc…");
}
