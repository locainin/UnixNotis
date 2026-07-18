use super::super::text::*;

#[test]
fn home_redaction_requires_a_complete_path_boundary() {
    let home = std::env::var("HOME").expect("HOME");
    let similar = format!("{home}ice/private");
    let contained = format!("{home}/private");

    assert_eq!(redact_home_text(&similar), similar);
    assert_eq!(redact_home_text(&contained), "$HOME/private");
}

#[test]
fn safe_text_removes_controls_redacts_home_and_stays_bounded() {
    let home = std::env::var("HOME").expect("HOME");
    let raw = format!("\u{1b}[31m{home}/private/{}", "x".repeat(2_000));

    let safe = safe_doctor_text(&raw);

    assert!(!safe.contains('\u{1b}'));
    assert!(!safe.contains(&home));
    assert!(safe.starts_with("$HOME/private/"));
    assert!(safe.chars().count() <= DOCTOR_DETAIL_CHAR_LIMIT);
}

#[test]
fn safe_text_removes_complete_csi_and_osc_sequences() {
    let safe = safe_doctor_text(
        "\u{1b}[31mred\u{1b}[0m \u{1b}]0;secret\u{7}plain \
         \u{1b}]8;;hidden\u{1b}\\link \u{9b}32mgreen\u{9b}0m",
    );

    assert_eq!(safe, "red plain link green");
}

#[test]
fn truncation_respects_character_budget_for_unicode() {
    assert_eq!(truncate_with_ellipsis("abcdef", 4), "abc…");
    assert_eq!(truncate_with_ellipsis("ééé", 2), "é…");
    assert_eq!(truncate_with_ellipsis("é", 1), "é");
    assert_eq!(truncate_with_ellipsis("é", 0), "");
}
