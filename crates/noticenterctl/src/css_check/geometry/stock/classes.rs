use std::collections::HashSet;
use std::sync::OnceLock;

use unixnotis_core::{
    css::hooks, DEFAULT_BASE_CSS, DEFAULT_MEDIA_CSS, DEFAULT_PANEL_CSS, DEFAULT_POPUP_CSS,
    DEFAULT_WIDGETS_CSS,
};

pub(in crate::main_css_check) fn known_unixnotis_classes() -> &'static HashSet<String> {
    static CLASSES: OnceLock<HashSet<String>> = OnceLock::new();
    CLASSES.get_or_init(|| {
        let mut classes = HashSet::new();
        // Scan the shipped theme once and reuse the set for every lint run
        for css in [
            DEFAULT_BASE_CSS,
            DEFAULT_PANEL_CSS,
            DEFAULT_POPUP_CSS,
            DEFAULT_WIDGETS_CSS,
            DEFAULT_MEDIA_CSS,
        ] {
            collect_unixnotis_classes(css, &mut classes);
        }
        // Hook-only classes are real public selectors even before stock CSS uses them
        // Keep this wired to shared constants so css-check follows the runtime tree
        for class_name in hook_unixnotis_classes() {
            insert_hook_class(&mut classes, class_name);
        }
        classes
    })
}

fn collect_unixnotis_classes(css: &'static str, classes: &mut HashSet<String>) {
    let bytes = css.as_bytes();
    let mut index = 0usize;
    while index < bytes.len() {
        if bytes[index] != b'.' || !css[index + 1..].starts_with("unixnotis-") {
            index += 1;
            continue;
        }

        let start = index;
        index += 1;
        while index < bytes.len() {
            let byte = bytes[index];
            if !(byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_') {
                break;
            }
            index += 1;
        }

        // Store selectors with the leading dot because parser checks use CSS selector text
        classes.insert(css[start..index].to_string());
    }
}

fn insert_hook_class(classes: &mut HashSet<String>, class_name: &str) {
    // Runtime hooks omit the CSS dot; css-check stores selector-form class names
    classes.insert(format!(".{class_name}"));
}

fn hook_unixnotis_classes() -> &'static [&'static str] {
    // Hook-only classes can be real live selectors before the stock theme gives them rules
    &[
        hooks::panel_action::ROW,
        hooks::panel_action::GROUP,
        hooks::panel_action::ROOT,
        hooks::panel_action::CONTENT,
        hooks::panel_action::GLYPH,
        hooks::panel_action::LABEL,
        hooks::panel_action::FOCUS,
        hooks::panel_action::PRIMARY,
        hooks::panel_action::MUTED,
        hooks::panel_action::SEARCH,
        hooks::panel_action::CLOSE,
        hooks::panel_action::WITH_ICON,
        hooks::panel_action::ICON_ONLY,
        hooks::panel_action::LABEL_HIDDEN,
        hooks::panel_shell::SUBTITLE,
        hooks::panel_shell::SEARCH_SHELL,
        hooks::panel_shell::SEARCH_ACCENT,
        hooks::panel_shell::SEARCH_STAR,
        hooks::panel_shell::BODY_STACK,
        hooks::panel_shell::EDGE_TOP,
        hooks::panel_shell::EDGE_BOTTOM,
        hooks::panel_shell::RAIL_LEFT,
        hooks::panel_shell::RAIL_RIGHT,
        hooks::panel_shell::TICK_TOP_LEFT,
        hooks::panel_shell::TICK_TOP_RIGHT,
        hooks::panel_shell::TICK_BOTTOM_LEFT,
        hooks::panel_shell::TICK_BOTTOM_RIGHT,
        hooks::panel_shell::SECTION_HEADER,
        hooks::panel_shell::RECENT_SECTION,
        hooks::panel_shell::RECENT_HEADER,
        hooks::panel_shell::RECENT_HEADER_ROW,
        hooks::panel_shell::FOOTER,
        hooks::panel_card::HEADER,
        hooks::panel_card::TEXT,
        hooks::panel_card::META_TOP,
        hooks::panel_card::META_LABEL,
        hooks::panel_card::TIME_BADGE,
        hooks::panel_card::FOOTER,
        hooks::panel_card::FOOTER_LEFT,
        hooks::panel_card::FOOTER_RIGHT,
        hooks::panel_card::THUMBNAIL,
        hooks::panel_card::HAS_THUMBNAIL,
        hooks::panel_card::NO_THUMBNAIL,
        hooks::slider::STACK,
        hooks::slider::SEGMENTS,
        hooks::slider::SEGMENT,
        hooks::slider::SUBLABEL_ROW,
        hooks::slider::SUBLABEL_MIN,
        hooks::slider::SUBLABEL_MAX,
        hooks::info_card::MEDIA,
        hooks::info_card::CHROME,
        hooks::info_card::DOTS,
        hooks::info_card::DOT,
        hooks::info_card::NAV_PREV,
        hooks::info_card::NAV_NEXT,
        hooks::info_card::LAYOUT_BANNER,
        hooks::info_card::LAYOUT_IMAGE_ROW,
        hooks::group_row::ROOT,
        hooks::group_row::CONTAINER,
        hooks::group_row::HEADER,
        hooks::group_row::ICON,
        hooks::group_row::TITLE,
        hooks::group_row::COUNT,
        hooks::group_row::CHEVRON,
        hooks::empty_row::ROOT,
        hooks::empty_row::LABEL,
        hooks::ghost_row::ROOT,
        "unixnotis-stack-ghost-1",
        "unixnotis-stack-ghost-2",
        "unixnotis-media-stack-player",
        "unixnotis-media-row-player",
        "unixnotis-media-card-player",
        "unixnotis-media-button-prev",
        "unixnotis-media-button-play",
        "unixnotis-media-button-next",
    ]
}

#[cfg(test)]
mod tests {
    use super::known_unixnotis_classes;

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
}
