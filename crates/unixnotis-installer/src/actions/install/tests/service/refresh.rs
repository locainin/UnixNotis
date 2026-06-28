use super::super::super::service::{
    s6_stderr_diagnostic, sanitize_diagnostic_line, truncate_diagnostic,
};

#[test]
fn s6_update_diagnostic_strips_control_sequences() {
    let diagnostic = s6_stderr_diagnostic(b"\x1b[31mfailed\x1b[0m\tbad\r\n")
        .expect("diagnostic should remain after sanitizing controls");

    // Escape bytes and carriage returns should not be able to reach the TUI log path
    assert_eq!(diagnostic, "[31mfailed[0m bad");
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

    // The helper must not split the two-byte `é` while enforcing the byte budget
    assert_eq!(diagnostic, format!("{}...", "a".repeat(239)));
}

#[test]
fn sanitize_diagnostic_line_trims_and_converts_tabs_to_spaces() {
    let sanitized = sanitize_diagnostic_line(" \talpha\tbeta\r\n");

    // Tabs become normal spaces so compact log layouts keep stable columns
    assert_eq!(sanitized, "alpha beta");
}
