use super::format_cli_error;

#[test]
fn cli_error_rendering_sanitizes_controls_and_keeps_context_chain() {
    let error = anyhow::anyhow!("payload\u{1b}[2Jfailed\nagain").context("import failed");

    let rendered = format_cli_error(&error);

    assert!(rendered.starts_with("Error: import failed: payload [2Jfailed again"));
    assert!(!rendered.contains('\u{1b}'));
    assert_eq!(rendered.lines().count(), 1);
}

#[test]
fn cli_error_rendering_bounds_very_large_attacker_text() {
    let error = anyhow::anyhow!("{}", "x".repeat(8_192)).context("read preset");

    let rendered = format_cli_error(&error);

    assert!(rendered.len() < 4_200);
    assert!(rendered.ends_with("...\n"));
}
