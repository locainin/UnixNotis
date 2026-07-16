use super::super::sanitize::*;

#[test]
fn journal_limits_match_the_public_diagnostics_contract() {
    assert_eq!(JOURNAL_LINE_LIMIT, 30);
    assert_eq!(JOURNAL_TOTAL_BYTE_LIMIT, 32_768);
    assert_eq!(JOURNAL_LINE_CHAR_LIMIT, 512);
    assert!(!journal_output_exceeds_limit(32_768));
    assert!(journal_output_exceeds_limit(32_769));
}

#[test]
fn journal_output_is_sanitized_truncated_and_line_bounded() {
    let home = std::env::var("HOME").expect("HOME");
    let mut raw = String::new();
    for _ in 0..(JOURNAL_LINE_LIMIT + 10) {
        raw.push_str("unsafe\u{1b}[31m");
        raw.push_str(&home);
        raw.push('/');
        raw.push_str(&"x".repeat(JOURNAL_LINE_CHAR_LIMIT + 100));
        raw.push('\n');
    }
    let collection = sanitize_journal(raw.as_bytes());

    assert_eq!(collection.lines.len(), JOURNAL_LINE_LIMIT);
    assert!(collection.content_truncated);
    assert!(collection.was_truncated());
    assert!(collection.lines.iter().all(|line| !line.contains('\u{1b}')));
    assert!(collection.lines.iter().all(|line| !line.contains(&home)));
    assert!(collection
        .lines
        .iter()
        .all(|line| line.chars().count() <= JOURNAL_LINE_CHAR_LIMIT));
    assert!(collection.lines.iter().all(|line| line.ends_with('…')));
}

#[test]
fn journal_content_limits_report_each_boundary_independently() {
    let exact_lines = "short\n".repeat(JOURNAL_LINE_LIMIT);
    let extra_line = "short\n".repeat(JOURNAL_LINE_LIMIT + 1);
    let exact_chars = "x".repeat(JOURNAL_LINE_CHAR_LIMIT);
    let extra_char = "x".repeat(JOURNAL_LINE_CHAR_LIMIT + 1);

    assert!(!sanitize_journal(exact_lines.as_bytes()).content_truncated);
    assert!(sanitize_journal(extra_line.as_bytes()).content_truncated);
    assert!(!sanitize_journal(exact_chars.as_bytes()).content_truncated);
    assert!(sanitize_journal(extra_char.as_bytes()).content_truncated);
}

#[test]
fn journal_truncation_combines_content_and_byte_reasons() {
    for (content_truncated, byte_truncated, expected) in [
        (false, false, false),
        (true, false, true),
        (false, true, true),
        (true, true, true),
    ] {
        let collection = JournalCollection {
            lines: Vec::new(),
            content_truncated,
            byte_truncated,
        };
        assert_eq!(collection.was_truncated(), expected);
    }
}
