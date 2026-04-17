use std::collections::HashSet;
use std::sync::OnceLock;

use unixnotis_core::{
    DEFAULT_BASE_CSS, DEFAULT_MEDIA_CSS, DEFAULT_PANEL_CSS, DEFAULT_POPUP_CSS, DEFAULT_WIDGETS_CSS,
};

pub(in crate::main_css_check) fn known_unixnotis_classes() -> &'static HashSet<&'static str> {
    static CLASSES: OnceLock<HashSet<&'static str>> = OnceLock::new();
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
        // Hook-only classes are still real live widget classes even before the stock theme styles them
        classes.extend(hook_unixnotis_classes());
        classes
    })
}

fn collect_unixnotis_classes(css: &'static str, classes: &mut HashSet<&'static str>) {
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

        // Borrow slices from the static CSS string so no extra allocation is needed
        classes.insert(&css[start..index]);
    }
}

fn hook_unixnotis_classes() -> [&'static str; 27] {
    // Hook-only classes can be real live selectors before the stock theme gives them rules
    [
        ".unixnotis-panel-actions",
        ".unixnotis-panel-action-group",
        ".unixnotis-panel-action",
        ".unixnotis-panel-action-content",
        ".unixnotis-panel-action-glyph",
        ".unixnotis-panel-action-label",
        ".unixnotis-panel-action-focus",
        ".unixnotis-panel-action-primary",
        ".unixnotis-panel-action-muted",
        ".unixnotis-panel-action-search",
        ".unixnotis-panel-action-close",
        ".unixnotis-panel-action-with-icon",
        ".unixnotis-panel-action-icon",
        ".unixnotis-group",
        ".unixnotis-group-row",
        ".unixnotis-group-header",
        ".unixnotis-group-icon",
        ".unixnotis-group-title",
        ".unixnotis-group-count",
        ".unixnotis-group-chevron",
        ".unixnotis-empty",
        ".unixnotis-media-stack-player",
        ".unixnotis-media-row-player",
        ".unixnotis-media-card-player",
        ".unixnotis-media-button-prev",
        ".unixnotis-media-button-play",
        ".unixnotis-media-button-next",
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
}
