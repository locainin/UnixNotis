use unixnotis_core::{INHIBIT_SCOPE_ALL, INHIBIT_SCOPE_POPUPS};

use super::super::sanitize::{normalize_inhibit_scope, sanitize_inhibit_reason};

#[test]
fn sanitize_inhibit_reason_trims_empty_reason_to_manual() {
    // Blank reasons should not render as empty rows in the panel
    assert_eq!(sanitize_inhibit_reason("   "), "manual");
}

#[test]
fn sanitize_inhibit_reason_truncates_without_splitting_utf8() {
    let long = format!("{}🙂", "a".repeat(512));

    let bounded = sanitize_inhibit_reason(&long);

    assert!(bounded.len() <= 256);
    assert!(bounded.is_char_boundary(bounded.len()));
    assert_eq!(bounded, "a".repeat(256));
}

#[test]
fn sanitize_inhibit_reason_keeps_exact_byte_limit_unchanged() {
    let exact = "a".repeat(256);

    assert_eq!(sanitize_inhibit_reason(&exact), exact);
}

#[test]
fn sanitize_inhibit_reason_removes_partial_multibyte_tail() {
    let reason = format!("{}🙂", "a".repeat(255));

    let bounded = sanitize_inhibit_reason(&reason);

    assert_eq!(bounded, "a".repeat(255));
    assert!(bounded.is_char_boundary(bounded.len()));
}

#[test]
fn normalize_inhibit_scope_accepts_supported_values() {
    // Scope "all" is a full override and should remain zero
    assert_eq!(
        normalize_inhibit_scope(INHIBIT_SCOPE_ALL).expect("scope"),
        INHIBIT_SCOPE_ALL
    );
    assert_eq!(
        normalize_inhibit_scope(INHIBIT_SCOPE_POPUPS).expect("scope"),
        INHIBIT_SCOPE_POPUPS
    );
}

#[test]
fn normalize_inhibit_scope_rejects_values_without_supported_bits() {
    assert!(normalize_inhibit_scope(2).is_err());
    assert!(normalize_inhibit_scope(!INHIBIT_SCOPE_POPUPS).is_err());
}

#[test]
fn normalize_inhibit_scope_strips_unknown_bits_when_popup_bit_is_present() {
    let mixed = INHIBIT_SCOPE_POPUPS | 0b1000;

    let normalized = normalize_inhibit_scope(mixed).expect("mixed scope");

    assert_eq!(normalized, INHIBIT_SCOPE_POPUPS);
}
