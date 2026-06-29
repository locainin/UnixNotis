use super::super::super::service::{
    s6_stderr_diagnostic, sanitize_diagnostic_line, strip_ansi_csi_sequences, truncate_diagnostic,
};

#[test]
fn s6_update_diagnostic_strips_control_bytes() {
    let diagnostic = s6_stderr_diagnostic(b"\x1b[31mfailed\x1b[0m\tbad\r\n")
        .expect("diagnostic should remain after sanitizing controls");

    // Escape controls and their CSI payloads are removed before compact log rendering
    assert_eq!(diagnostic, "failed bad");
}

#[test]
fn s6_update_diagnostic_keeps_first_non_empty_line() {
    let diagnostic = s6_stderr_diagnostic(b"\r\n\t\n  first useful line  \nsecond line\n")
        .expect("first non-empty sanitized line should be selected");

    // Empty lines after trimming and tab normalization should be skipped
    assert_eq!(diagnostic, "first useful line");
}

#[test]
fn s6_update_diagnostic_truncates_utf8_safely() {
    let source = format!("{}étail", "a".repeat(239));
    let diagnostic = truncate_diagnostic(source, 240);

    // The helper must not split the two-byte `é`, and the ellipsis stays inside the byte budget
    assert_eq!(diagnostic, format!("{}...", "a".repeat(237)));
    assert!(diagnostic.len() <= 240);
}

#[test]
fn s6_update_diagnostic_handles_tiny_truncation_budget() {
    let diagnostic = truncate_diagnostic("abcdef".to_string(), 2);

    // Tiny limits still produce valid UTF-8 and never exceed the requested byte budget
    assert_eq!(diagnostic, "..");
}

#[test]
fn sanitize_diagnostic_line_trims_and_converts_tabs_to_spaces() {
    let sanitized = sanitize_diagnostic_line(" \talpha\tbeta\r\n");

    // Tabs become normal spaces so compact log layouts keep stable columns
    assert_eq!(sanitized, "alpha beta");
}

#[test]
fn ansi_csi_stripper_removes_color_sequences_without_touching_text() {
    let sanitized = strip_ansi_csi_sequences("\x1b[31mred\x1b[0m plain");

    // CSI color escapes should not leave bracket fragments in the user-facing diagnostic
    assert_eq!(sanitized, "red plain");
}
