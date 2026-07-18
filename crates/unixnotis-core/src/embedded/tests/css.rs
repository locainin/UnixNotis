use super::{
    DEFAULT_BASE_CSS, DEFAULT_MEDIA_CSS, DEFAULT_PANEL_CSS, DEFAULT_POPUP_CSS, DEFAULT_WIDGETS_CSS,
    INTERNAL_STRUCTURE_CSS,
};

#[test]
fn every_embedded_css_layer_contains_real_stylesheet_content() {
    for (name, css) in [
        ("base", DEFAULT_BASE_CSS),
        ("panel", DEFAULT_PANEL_CSS),
        ("popup", DEFAULT_POPUP_CSS),
        ("widgets", DEFAULT_WIDGETS_CSS),
        ("media", DEFAULT_MEDIA_CSS),
        ("internal structure", INTERNAL_STRUCTURE_CSS),
    ] {
        assert!(!css.trim().is_empty(), "{name} CSS should not be empty");
    }
}

#[test]
fn internal_structure_css_only_targets_reload_notice_structure() {
    assert!(INTERNAL_STRUCTURE_CSS.contains(".unixnotis-reload-notice"));
    assert!(!INTERNAL_STRUCTURE_CSS.contains("@define-color"));
}
