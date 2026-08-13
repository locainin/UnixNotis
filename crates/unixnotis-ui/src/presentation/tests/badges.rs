use super::super::{build_semantic_badge, BadgePresentation};

#[gtk::test]
fn uncertain_identity_badges_load_from_controlled_resources() {
    for badge in [
        BadgePresentation::UnknownApplication,
        BadgePresentation::SuspiciousApplication,
        BadgePresentation::CommandLine,
        BadgePresentation::System,
    ] {
        let image = build_semantic_badge(badge, 20).expect("semantic badge should exist");

        let icon_name = image.icon_name().expect("named badge icon");
        assert!(icon_name.starts_with("unixnotis-"));
        let display = gtk::gdk::Display::default().expect("GTK display");
        assert!(
            gtk::IconTheme::for_display(&display).has_icon(&icon_name),
            "badge should resolve through the named symbolic icon theme"
        );
        assert_eq!(image.pixel_size(), 20);
    }
}

#[gtk::test]
fn authenticated_identity_keeps_the_application_badge() {
    assert!(build_semantic_badge(BadgePresentation::AuthenticatedApplication, 20).is_none());
}
