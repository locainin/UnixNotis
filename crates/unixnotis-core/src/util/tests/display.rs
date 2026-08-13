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

#[test]
fn layout_folding_bounds_long_unbroken_tokens() {
    let input = "x".repeat(200);
    let folded = fold_text_for_layout(&input, MAX_DISPLAY_TOKEN_WIDTH);
    let longest = folded
        .split_whitespace()
        .map(|part| part.chars().filter(char::is_ascii_alphanumeric).count())
        .max()
        .unwrap_or(0);

    assert!(folded.contains('…'));
    assert!(longest <= MAX_DISPLAY_TOKEN_WIDTH);
}

#[test]
fn layout_folding_handles_zero_exact_and_separate_token_limits() {
    assert_eq!(fold_text_for_layout("unchanged", 0), "unchanged");
    assert_eq!(
        fold_text_for_layout(
            &"x".repeat(MAX_DISPLAY_TOKEN_WIDTH),
            MAX_DISPLAY_TOKEN_WIDTH
        ),
        "x".repeat(MAX_DISPLAY_TOKEN_WIDTH)
    );
    let separate = format!(
        "{} {}",
        "x".repeat(MAX_DISPLAY_TOKEN_WIDTH),
        "y".repeat(MAX_DISPLAY_TOKEN_WIDTH)
    );
    assert_eq!(
        fold_text_for_layout(&separate, MAX_DISPLAY_TOKEN_WIDTH),
        separate
    );
}

#[test]
fn layout_folding_reserves_only_the_required_ellipsis_width() {
    assert_eq!(fold_text_for_layout("xxxx", 3), "x…");
    let folded = fold_text_for_layout(&"x".repeat(200), MAX_DISPLAY_TOKEN_WIDTH);

    // The CJK-width ellipsis uses two columns beside 94 ASCII characters
    assert_eq!(folded.chars().count(), 95);
}

#[test]
fn layout_folding_counts_wide_glyphs_joiners_and_selectors() {
    let wide = fold_text_for_layout(&"界".repeat(120), MAX_DISPLAY_TOKEN_WIDTH);
    let emoji = fold_text_for_layout(
        &"👨\u{200D}👩\u{200D}👧\u{200D}👦".repeat(80),
        MAX_DISPLAY_TOKEN_WIDTH,
    );

    assert!(wide.chars().map(display_width).sum::<usize>() <= MAX_DISPLAY_TOKEN_WIDTH);
    assert!(emoji.chars().map(display_width).sum::<usize>() <= MAX_DISPLAY_TOKEN_WIDTH);
    assert!(display_width('界') > 1);
    assert_eq!(display_width('\u{200D}'), 1);
}
