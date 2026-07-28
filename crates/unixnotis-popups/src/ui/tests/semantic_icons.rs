use super::super::entry::TrustLevel;
use super::super::semantic_icons::build_semantic_badge;

#[gtk::test]
fn uncertain_trust_states_use_bundled_semantic_resources() {
    super::super::super::app::resources::register().expect("register popup resources");

    for level in [
        TrustLevel::Unverified,
        TrustLevel::Suspicious,
        TrustLevel::System,
    ] {
        let image = build_semantic_badge(level, 20).expect("semantic badge");

        assert!(image.paintable().is_some(), "bundled badge should load");
        assert_eq!(image.pixel_size(), 20);
    }
}

#[gtk::test]
fn verified_identity_does_not_replace_the_authenticated_badge() {
    assert!(build_semantic_badge(TrustLevel::Verified, 20).is_none());
}
