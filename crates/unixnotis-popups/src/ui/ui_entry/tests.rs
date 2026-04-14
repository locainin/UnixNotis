//! Popup entry helper tests
//!
//! Covers the tiny layout rules that do not need full GTK setup

use super::labels::{optional_label_state, POPUP_BODY_MAX_CHARS, POPUP_SUMMARY_MAX_CHARS};
use super::popup_header_spacer_expands;

#[test]
fn popup_header_spacer_expands_to_hold_close_alignment() {
    // The tested rule is the important part, not the GTK object itself
    assert!(popup_header_spacer_expands());
}

#[test]
fn popup_summary_row_hides_when_text_is_empty() {
    // Empty summaries should not reserve vertical space above the body
    let state = optional_label_state("", POPUP_SUMMARY_MAX_CHARS);

    assert!(!state.visible);
    assert!(state.text.is_empty());
}

#[test]
fn popup_body_row_hides_when_text_is_empty() {
    // Body-less notifications should render as header plus summary only
    let state = optional_label_state("", POPUP_BODY_MAX_CHARS);

    assert!(!state.visible);
    assert!(state.text.is_empty());
}

#[test]
fn popup_body_row_hides_when_text_is_only_whitespace() {
    // Space-only bodies should not leave a blank band in the popup card
    let state = optional_label_state("\n\t ", POPUP_BODY_MAX_CHARS);

    assert!(!state.visible);
    assert!(state.text.is_empty());
}

#[test]
fn popup_summary_row_shows_when_text_has_real_content() {
    // Real text should stay intact even when it has leading whitespace
    let state = optional_label_state("  hello  ", POPUP_SUMMARY_MAX_CHARS);

    assert!(state.visible);
    assert_eq!(state.text.as_ref(), "  hello  ");
}
