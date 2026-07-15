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
