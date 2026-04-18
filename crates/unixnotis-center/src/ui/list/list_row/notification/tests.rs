//! Small notification-row tests that do not need full GTK setup
//!
//! Keeping these beside the row module makes the text rules easier to maintain

use super::state::MAX_SUMMARY_LABEL_CHARS;
use super::update::{clamp_action_label_text, optional_label_state};

#[test]
fn panel_summary_row_hides_when_text_is_empty() {
    // Empty summaries should not leave a blank strip above the body
    let state = optional_label_state("", MAX_SUMMARY_LABEL_CHARS);

    assert!(!state.visible);
    assert!(state.text.is_empty());
}

#[test]
fn panel_summary_row_hides_when_text_is_only_whitespace() {
    // Space-only payloads should collapse the same as truly empty payloads
    let state = optional_label_state("\n\t ", MAX_SUMMARY_LABEL_CHARS);

    assert!(!state.visible);
    assert!(state.text.is_empty());
}

#[test]
fn panel_summary_row_shows_when_text_has_real_content() {
    // Leading and trailing space should not hide actual notification text
    let state = optional_label_state("  hello  ", MAX_SUMMARY_LABEL_CHARS);

    assert!(state.visible);
    assert_eq!(state.text.as_ref(), "  hello  ");
}

#[test]
fn panel_summary_row_hides_when_clamp_intentionally_blanks_text() {
    // Zero-char clamps should collapse the row instead of leaving an empty label
    let state = optional_label_state("hello", 0);

    assert!(!state.visible);
    assert!(state.text.is_empty());
}

#[test]
fn panel_action_labels_are_clamped_before_button_build() {
    // Long labels should be shortened before the action row sees them
    let long_label = "This action label is much longer than the row should allow";
    let rendered = clamp_action_label_text(long_label);

    assert!(rendered.len() < long_label.len());
    assert!(rendered.ends_with('…'));
}
