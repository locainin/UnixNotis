use super::{
    DEFAULT_BASE_CSS, DEFAULT_MEDIA_CSS, DEFAULT_PANEL_CSS, DEFAULT_POPUP_CSS, DEFAULT_WIDGETS_CSS,
    INTERNAL_STRUCTURE_CSS, MOTION_POLICY_CSS,
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
        ("motion policy", MOTION_POLICY_CSS),
    ] {
        assert!(!css.trim().is_empty(), "{name} CSS should not be empty");
    }
}

#[test]
fn motion_policy_disables_theme_motion_under_the_runtime_class() {
    assert!(MOTION_POLICY_CSS.contains(".unixnotis-panel.unixnotis-reduced-motion"));
    assert!(MOTION_POLICY_CSS.contains("transition: none"));
    assert!(MOTION_POLICY_CSS.contains("animation: none"));
    assert!(MOTION_POLICY_CSS.contains("transform: none"));
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

#[test]
fn stock_panel_hover_styles_avoid_transform_and_geometry_animation() {
    for (name, css) in [
        ("panel", DEFAULT_PANEL_CSS),
        ("widgets", DEFAULT_WIDGETS_CSS),
        ("media", DEFAULT_MEDIA_CSS),
    ] {
        assert!(
            !css.contains("\n  transform:"),
            "{name} CSS should not move widgets during hover"
        );
    }

    assert!(!DEFAULT_PANEL_CSS.contains("transition: background-image"));
    assert!(!DEFAULT_WIDGETS_CSS.contains("transition: min-width"));
    assert!(!DEFAULT_WIDGETS_CSS.contains("transition: min-height"));
}

#[test]
fn stock_scrollbar_keeps_master_sizing_without_geometry_animation() {
    assert!(DEFAULT_PANEL_CSS.contains(
        "scrollbar slider {\n  background: alpha(#ffffff, 0.16);\n  border-radius: 999px;\n  border: none;\n  min-width: 4px;"
    ));
    for selector in ["scrollbar slider:hover", "scrollbar slider:active"] {
        let rule = DEFAULT_PANEL_CSS
            .split(selector)
            .nth(1)
            .and_then(|suffix| suffix.split('}').next())
            .expect("stock scrollbar state rule");
        assert!(
            rule.contains("min-width: 6px"),
            "{selector} should retain the master width"
        );
    }
    assert!(!DEFAULT_PANEL_CSS.contains("transition: background-color 0.15s ease-out, min-width"));
}

#[test]
fn critical_alert_assets_define_composed_popup_and_panel_states() {
    for token in [
        "unixnotis-critical-surface",
        "unixnotis-critical-surface-strong",
        "unixnotis-critical-border",
        "unixnotis-critical-text",
        "unixnotis-critical-icon",
    ] {
        assert!(
            DEFAULT_BASE_CSS.contains(token),
            "base CSS should define {token}"
        );
    }

    for selector in [
        ".unixnotis-popup-card.critical",
        ".unixnotis-popup-card.critical .unixnotis-popup-icon",
        ".unixnotis-panel-card.critical,\n.unixnotis-panel-card.active.critical",
        ".unixnotis-panel-card.critical .unixnotis-panel-icon",
    ] {
        let css = if selector.contains("popup") {
            DEFAULT_POPUP_CSS
        } else {
            DEFAULT_PANEL_CSS
        };
        assert!(css.contains(selector), "stock CSS should retain {selector}");
    }

    assert!(DEFAULT_BASE_CSS.contains(".unixnotis-urgency-badge"));
    assert!(!DEFAULT_PANEL_CSS.contains("animation:"));
    assert!(!DEFAULT_POPUP_CSS.contains("animation:"));
}

#[test]
fn popup_theme_keeps_kind_trust_and_compact_media_hooks() {
    for selector in [
        ".unixnotis-popup-card.utility",
        ".unixnotis-popup-communication-content",
        ".unixnotis-popup-utility-content",
        ".unixnotis-popup-trust-chip.recognized",
        ".unixnotis-popup-trust-chip.unresolved",
        ".unixnotis-popup-trust-chip.relay",
        ".unixnotis-popup-trust-chip.conflict",
        ".unixnotis-popup-time",
    ] {
        assert!(
            DEFAULT_POPUP_CSS.contains(selector),
            "popup CSS should retain {selector}"
        );
    }

    // Default popups must not restore the old raw provenance body row
    assert!(!DEFAULT_POPUP_CSS.contains(".unixnotis-popup-source"));
    assert!(DEFAULT_POPUP_CSS.contains(".unixnotis-identity-avatar"));
    assert!(DEFAULT_POPUP_CSS.contains("min-width: 46px"));
    assert!(DEFAULT_POPUP_CSS.contains("min-width: 64px"));
    assert!(!DEFAULT_POPUP_CSS.contains("popup-warning-content"));
}

#[test]
fn notification_surfaces_keep_compact_master_geometry() {
    assert!(DEFAULT_PANEL_CSS.contains("var(--unixnotis-notification-card-radius)"));
    assert!(DEFAULT_PANEL_CSS.contains("var(--unixnotis-panel-card-padding-y)"));
    assert!(DEFAULT_PANEL_CSS.contains("var(--unixnotis-panel-card-padding-x)"));
    assert!(DEFAULT_POPUP_CSS.contains("var(--unixnotis-popup-card-radius)"));
    assert!(DEFAULT_POPUP_CSS.contains("var(--unixnotis-popup-card-padding-y)"));
    assert!(DEFAULT_POPUP_CSS.contains("var(--unixnotis-popup-card-padding-x)"));
    assert!(DEFAULT_PANEL_CSS.contains("margin: -58px 14px 0"));
    assert!(DEFAULT_PANEL_CSS.contains("margin: 0 20px"));
}

#[test]
fn media_cards_keep_art_and_transport_as_separate_visual_lanes() {
    assert!(DEFAULT_MEDIA_CSS.contains(".unixnotis-media-art-frame"));
    assert!(DEFAULT_MEDIA_CSS.contains(".unixnotis-media-control-strip"));
    assert!(DEFAULT_MEDIA_CSS.contains(".unixnotis-media-card.playing"));
    assert!(DEFAULT_MEDIA_CSS.contains("--unixnotis-media-art-size"));
}
