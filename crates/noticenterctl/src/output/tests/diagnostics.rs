use super::format_notification_diagnostics;

#[test]
fn diagnostics_keep_launch_verification_distinct_from_attribution_status() {
    let output =
        format_notification_diagnostics(&unixnotis_core::NotificationDiagnosticsView::default())
            .expect("default diagnostics should render");

    assert!(
        output.contains("Launch verification: unverified"),
        "diagnostics should name the launch evidence being reported"
    );
    assert!(
        output.contains("Launch detail: none"),
        "diagnostics should label the launch evidence detail"
    );
    assert!(
        output.contains("Identity assurance: unresolved"),
        "final identity authority must remain distinct from the launch match"
    );
    assert!(
        output.contains("Default activation: denied")
            && output.contains("Action buttons: denied")
            && output.contains("Inline reply: denied"),
        "diagnostics must expose every independent interaction policy"
    );
}

#[test]
fn diagnostics_sanitize_sender_controlled_terminal_text() {
    let mut view = unixnotis_core::NotificationDiagnosticsView::default();

    view.attribution.claimed_name =
        "Example App\nFORGED_DIAGNOSTIC_LINE:\u{1b}[31mred\u{1b}[0m".to_string();
    view.attribution.claimed_desktop_entry =
        "org.example.App.desktop\nFORGED_DESKTOP_LINE".to_string();
    view.attribution.sender_executable =
        "/tmp/example\nFORGED_EXECUTABLE_LINE:\u{1b}[2J".to_string();
    view.attribution.matched_desktop_id =
        "org.example.Match.desktop\nFORGED_MATCH_LINE".to_string();
    view.attribution.reason = "ambiguous\nFORGED_REASON_LINE:\u{202e}spoof".to_string();

    let output = format_notification_diagnostics(&view).expect("diagnostics should render");

    assert!(
        !output.contains('\u{1b}'),
        "terminal escape characters must not survive diagnostic rendering"
    );
    assert!(!output.contains('\u{202e}'));
    for forged_line in [
        "\nFORGED_DIAGNOSTIC_LINE:",
        "\nFORGED_DESKTOP_LINE",
        "\nFORGED_EXECUTABLE_LINE:",
        "\nFORGED_MATCH_LINE",
        "\nFORGED_REASON_LINE:",
    ] {
        assert!(
            !output.contains(forged_line),
            "diagnostic values must not inject terminal lines"
        );
    }
    assert!(
        output.contains("Application claim: Example App FORGED_DIAGNOSTIC_LINE:"),
        "sanitized diagnostic content should remain useful to the operator"
    );
    assert!(
        output.contains("Claimed desktop entry: org.example.App.desktop FORGED_DESKTOP_LINE"),
        "sanitized desktop-entry content should remain inspectable"
    );
    assert!(output.contains("Sender executable: /tmp/example FORGED_EXECUTABLE_LINE:"));
    assert!(output.contains("Matched desktop ID: org.example.Match.desktop FORGED_MATCH_LINE"));
    assert!(output.contains("Launch detail: ambiguous FORGED_REASON_LINE:spoof"));
}
