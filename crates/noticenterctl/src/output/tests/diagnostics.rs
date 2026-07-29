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
        !output.contains("Identity result:"),
        "launch evidence must not masquerade as the final attribution state"
    );
}
