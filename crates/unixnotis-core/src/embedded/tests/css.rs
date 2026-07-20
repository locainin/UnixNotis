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
fn internal_structure_css_contains_only_required_fallback_structure() {
    assert!(INTERNAL_STRUCTURE_CSS.contains(".unixnotis-reload-notice"));
    assert!(INTERNAL_STRUCTURE_CSS.contains(".unixnotis-panel-search-owned-icons"));
    assert!(!INTERNAL_STRUCTURE_CSS.contains("@define-color"));
}

#[test]
fn panel_css_keeps_the_dnd_menu_visual_hooks() {
    for selector in [
        ".unixnotis-dnd-menu > contents",
        ".unixnotis-dnd-menu-title",
        ".unixnotis-dnd-menu-choice",
        ".unixnotis-dnd-menu-choice-indefinite",
        ".unixnotis-dnd-menu-separator",
    ] {
        assert!(
            DEFAULT_PANEL_CSS.contains(selector),
            "panel CSS should retain {selector}"
        );
    }
}

#[test]
fn dnd_menu_hover_and_keyboard_focus_share_one_visual_rule() {
    let shared_selector = ".unixnotis-dnd-menu .unixnotis-dnd-menu-choice:hover,\n\
.unixnotis-dnd-menu .unixnotis-dnd-menu-choice:focus-visible";

    // PrintScreen can switch GTK into keyboard modality while the pointer remains over a row
    // One selector keeps that modality change from altering the captured menu appearance
    assert!(DEFAULT_PANEL_CSS.contains(shared_selector));
    assert!(!DEFAULT_PANEL_CSS.contains("box-shadow: inset 2px 0"));
}
