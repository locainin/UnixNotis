use super::*;

#[test]
fn sanitize_log_value_strips_newlines_and_caps() {
    let value = "ab\ncd\rEF";
    let sanitized = sanitize_log_value(value, 5);
    assert_eq!(sanitized, "ab cd...");

    let no_truncate = sanitize_log_value("ok", 5);
    assert_eq!(no_truncate, "ok");
}

#[test]
fn sanitize_log_value_strips_bidi_controls() {
    let value = "safe\u{202E}spoof\u{2066}text\u{2069}";
    let sanitized = sanitize_log_value(value, 80);
    assert_eq!(sanitized, "safespooftext");
}

#[test]
fn sanitize_log_value_replaces_newlines_and_other_controls() {
    let value = "a\nb\rc\u{0007}d";
    assert_eq!(sanitize_log_value(value, 80), "a b c d");
}

#[test]
fn sanitize_display_text_strips_bidi_controls_and_preserves_newlines() {
    // Multi-line UI text may keep rows, but spoofing direction marks are always removed
    let value = "safe\u{202E}name\nnext\u{2066}line\u{2069}";
    let sanitized = sanitize_display_text(value);
    assert_eq!(sanitized, "safename\nnextline");
}

#[test]
fn sanitize_inline_display_text_flattens_control_characters() {
    // Inline labels stay on one visual row even when input carries tabs or newlines
    let value = "fake\tname\nrow\u{202E}";
    let sanitized = sanitize_inline_display_text(value);
    assert_eq!(sanitized, "fake name row");
}

#[test]
fn sanitize_display_text_maps_tabs_and_carriage_returns_separately() {
    let value = "a\tb\rc\nnext";
    assert_eq!(sanitize_display_text(value), "a b c\nnext");
    assert_eq!(sanitize_inline_display_text(value), "a b c next");
}

#[test]
fn bounded_display_text_sanitizes_controls_without_scanning_the_full_value() {
    let value = "safe\u{001b}[2J\nnext\u{202e}spoof";

    let sanitized = sanitize_display_text_bounded(value, 10);

    assert_eq!(sanitized, "safe [2J\nn...");
    assert!(!sanitized.contains('\u{001b}'));
    assert!(!sanitized.contains('\u{202e}'));
}

#[test]
fn bounded_display_text_handles_zero_and_exact_limits() {
    assert_eq!(sanitize_display_text_bounded("value", 0), "");
    assert_eq!(sanitize_display_text_bounded("value", 5), "value...");
    assert_eq!(sanitize_display_text_bounded("ok", 5), "ok");
}
