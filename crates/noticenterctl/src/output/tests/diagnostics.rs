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
