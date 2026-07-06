use super::super::classes::{is_known_unixnotis_class, known_unixnotis_classes};

#[test]
fn player_button_hooks_are_treated_as_known_public_classes() {
    let classes = known_unixnotis_classes();

    assert!(classes.contains(".unixnotis-media-button-prev"));
    assert!(classes.contains(".unixnotis-media-button-play"));
    assert!(classes.contains(".unixnotis-media-button-next"));
}

#[test]
fn section_header_hooks_are_treated_as_known_public_classes() {
    let classes = known_unixnotis_classes();

    assert!(classes.contains(".unixnotis-section-header"));
    assert!(classes.contains(".unixnotis-recent-section"));
    assert!(classes.contains(".unixnotis-recent-header"));
    assert!(classes.contains(".unixnotis-recent-header-row"));
    assert!(classes.contains(".unixnotis-panel-footer"));
}

#[test]
fn notification_metadata_hooks_are_treated_as_known_public_classes() {
    let classes = known_unixnotis_classes();

    assert!(classes.contains(".unixnotis-panel-card-meta-top"));
    assert!(classes.contains(".unixnotis-panel-card-time-badge"));
    assert!(classes.contains(".unixnotis-panel-card-thumbnail"));
}

#[test]
fn decorative_theme_hooks_are_treated_as_known_public_classes() {
    let classes = known_unixnotis_classes();

    assert!(classes.contains(".unixnotis-panel-edge-top"));
    assert!(classes.contains(".unixnotis-panel-rail-left"));
    assert!(classes.contains(".unixnotis-panel-search-shell"));
    assert!(classes.contains(".unixnotis-quick-slider-segments"));
    assert!(classes.contains(".unixnotis-info-media"));
    assert!(classes.contains(".unixnotis-info-card-banner"));
    assert!(classes.contains(".unixnotis-panel-action-label-hidden"));
}

#[test]
fn dynamic_widget_kind_hooks_are_treated_as_known_public_classes() {
    assert!(is_known_unixnotis_class(".unixnotis-toggle-kind-wifi"));
    assert!(is_known_unixnotis_class(".unixnotis-stat-kind-ram"));
    assert!(is_known_unixnotis_class(
        ".unixnotis-info-card-kind-weather"
    ));
    assert!(!is_known_unixnotis_class(".unixnotis-stat-kind-"));
}
