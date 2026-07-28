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

        assert!(image.paintable().is_some(), "bundled badge should load");
        assert_eq!(image.pixel_size(), 20);
    }
}

#[gtk::test]
fn authenticated_identity_keeps_the_application_badge() {
    assert!(build_semantic_badge(BadgePresentation::AuthenticatedApplication, 20).is_none());
}
