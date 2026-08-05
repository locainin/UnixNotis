use super::super::collect_icon_candidates;
use super::support::notification;

#[test]
fn collect_icon_candidates_uses_only_daemon_associated_badge_variants() {
    let candidates =
        collect_icon_candidates(&notification("UnixNotis Center", "org.demo.App.desktop"));

    assert_eq!(
        candidates,
        vec![
            "org.demo.App.desktop",
            "org.demo.App",
            "org.demo.app.desktop",
        ]
    );
}

#[test]
fn collect_icon_candidates_includes_a_distinct_desktop_id() {
    let mut input = notification("UnixNotis Center", "trusted-badge");
    input.attribution.desktop_id = "org.demo.App.desktop".to_string();

    let candidates = collect_icon_candidates(&input);

    assert_eq!(
        candidates,
        vec![
            "trusted-badge",
            "org.demo.App.desktop",
            "org.demo.app.desktop",
        ]
    );
}

#[test]
fn collect_icon_candidates_dedupes_empty_and_repeated_values() {
    let candidates = collect_icon_candidates(&notification("App", "app"));

    assert_eq!(candidates, vec!["app"]);
}

#[test]
fn collect_icon_candidates_does_not_treat_content_icon_as_application_badge() {
    let mut notification = notification("authenticated-app", "trusted-badge");
    notification.image.sender_visual_role =
        unixnotis_core::NotificationVisualRole::ApplicationProvidedIcon;

    let candidates = collect_icon_candidates(&notification);

    assert!(candidates.iter().any(|value| value == "trusted-badge"));
    assert!(!candidates
        .iter()
        .any(|value| value == "caller-controlled-content"));
}

#[test]
fn collect_icon_candidates_keeps_a_bounded_unresolved_theme_hint_decorative() {
    let mut notification = notification("Trusted Brand", "dialog-warning-symbolic");
    notification.attribution.status = unixnotis_core::AttributionStatus::Unresolved;
    notification.attribution.desktop_id.clear();
    notification.attribution.claimed_name = "Trusted Brand".to_string();
    notification.image.claimed_theme_icon = "trusted-brand".to_string();

    let candidates = collect_icon_candidates(&notification);

    assert!(candidates
        .iter()
        .any(|value| value == "dialog-warning-symbolic"));
    assert!(candidates.iter().any(|value| value == "trusted-brand"));
    assert!(candidates.iter().all(|value| !value.contains('/')));
}
