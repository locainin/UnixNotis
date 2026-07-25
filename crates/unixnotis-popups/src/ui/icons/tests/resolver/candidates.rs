use super::super::collect_icon_candidates;
use super::support::notification;

#[test]
fn collect_icon_candidates_prefers_icon_name_variants_then_app_name_variants() {
    let candidates =
        collect_icon_candidates(&notification("UnixNotis Center", "org.demo.App.desktop"));

    assert_eq!(
        candidates,
        vec![
            "org.demo.App.desktop",
            "org.demo.App",
            "org.demo.app.desktop",
            "UnixNotis Center",
            "unixnotis center",
            "unixnotis-center",
        ]
    );
}

#[test]
fn collect_icon_candidates_dedupes_empty_and_repeated_values() {
    let candidates = collect_icon_candidates(&notification("App", "app"));

    assert_eq!(candidates, vec!["app", "App"]);
}

#[test]
fn collect_icon_candidates_does_not_treat_content_icon_as_application_badge() {
    let mut notification = notification("authenticated-app", "trusted-badge");
    notification.image.icon_name = "caller-controlled-content".to_string();

    let candidates = collect_icon_candidates(&notification);

    assert!(candidates.iter().any(|value| value == "trusted-badge"));
    assert!(!candidates
        .iter()
        .any(|value| value == "caller-controlled-content"));
}

#[test]
fn collect_icon_candidates_does_not_fallback_to_unresolved_brand_claim() {
    let mut notification = notification("Trusted Brand", "dialog-warning-symbolic");
    notification.attribution.verified = false;
    notification.attribution.reported_name.clear();

    let candidates = collect_icon_candidates(&notification);

    assert!(candidates
        .iter()
        .any(|value| value == "dialog-warning-symbolic"));
    assert!(!candidates.iter().any(|value| value == "Trusted Brand"));
}
