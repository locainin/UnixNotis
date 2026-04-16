//! Stock theme helpers for geometry lint

use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;

use unixnotis_core::{
    Config, DEFAULT_BASE_CSS, DEFAULT_MEDIA_CSS, DEFAULT_PANEL_CSS, DEFAULT_POPUP_CSS,
    DEFAULT_WIDGETS_CSS,
};

use super::super::main_css_check_parse::{
    next_css_block, normalize_selector, parse_css_declarations, split_selectors, strip_css_comments,
};
use super::super::main_css_check_policy::is_horizontal_size_property;
use super::model::GeometryModel;
use super::parse::collect_geometry_from_contents;

pub(super) fn known_unixnotis_classes() -> &'static HashSet<&'static str> {
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

pub(super) fn should_warn_for_unmodeled_known_class(
    class_name: &str,
    properties: &[(String, String)],
) -> bool {
    // This is the quiet-stock / loud-custom split for real UnixNotis hooks that do not have
    // direct width math yet
    // Any known live class that carries custom size rules should stay visible once it leaves
    // the shipped stock baseline
    !stock_matches_horizontal_size_rules(class_name, properties)
}

pub(super) fn stock_matches_complex_selector_rules(
    selector: &str,
    properties: &[(String, String)],
) -> bool {
    // Complex selectors need the same stock baseline guard so shipped descendant rules stay quiet
    let current_rules = normalized_horizontal_size_rules(properties);
    if current_rules.is_empty() {
        return true;
    }

    let Some(stock_rules) = stock_selector_horizontal_size_rules().get(selector) else {
        // Selectors with no stock baseline should stay visible when they drive width
        return false;
    };

    current_rules.iter().all(|(name, value)| {
        stock_rules
            .get(name.as_str())
            .map(|stock_value| stock_value == value)
            .unwrap_or(false)
    })
}

pub(super) fn stock_config() -> &'static Config {
    static CONFIG: OnceLock<Config> = OnceLock::new();
    // Default config is a stable baseline for false-positive control
    CONFIG.get_or_init(Config::default)
}

pub(super) fn stock_geometry_model() -> &'static GeometryModel {
    static MODEL: OnceLock<GeometryModel> = OnceLock::new();
    MODEL.get_or_init(|| {
        let mut model = GeometryModel::default();
        // Merge every shipped CSS file into one baseline geometry model
        for css in [
            DEFAULT_BASE_CSS,
            DEFAULT_PANEL_CSS,
            DEFAULT_POPUP_CSS,
            DEFAULT_WIDGETS_CSS,
            DEFAULT_MEDIA_CSS,
        ] {
            // The shipped theme is the baseline used to keep false positives low
            let _ = collect_geometry_from_contents(css, &mut model);
        }
        model
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

fn hook_unixnotis_classes() -> [&'static str; 21] {
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
    ]
}

fn stock_matches_horizontal_size_rules(class_name: &str, properties: &[(String, String)]) -> bool {
    let current_rules = normalized_horizontal_size_rules(properties);
    if current_rules.is_empty() {
        return true;
    }

    let Some(stock_rules) = stock_horizontal_size_rules().get(class_name) else {
        // No shipped baseline means any size rules on that class should stay visible
        return false;
    };

    // Matching every size rule in the current block keeps the shipped theme quiet
    current_rules.iter().all(|(name, value)| {
        stock_rules
            .get(name.as_str())
            .map(|stock_value| stock_value == value)
            .unwrap_or(false)
    })
}

fn normalized_horizontal_size_rules(properties: &[(String, String)]) -> HashMap<String, String> {
    let mut current_rules = HashMap::new();
    for (name, value) in properties
        .iter()
        .filter(|(name, _)| is_horizontal_size_property(name))
    {
        // The same selector can carry a literal fallback and a token override
        // The comparison needs the final value that GTK will keep
        // Duplicate properties use the later value, so the comparison needs the same rule
        current_rules.insert(name.trim().to_string(), value.trim().to_string());
    }
    current_rules
}

fn stock_horizontal_size_rules() -> &'static HashMap<String, HashMap<String, String>> {
    static RULES: OnceLock<HashMap<String, HashMap<String, String>>> = OnceLock::new();
    RULES.get_or_init(|| {
        let mut rules = HashMap::new();
        for css in [
            DEFAULT_BASE_CSS,
            DEFAULT_PANEL_CSS,
            DEFAULT_POPUP_CSS,
            DEFAULT_WIDGETS_CSS,
            DEFAULT_MEDIA_CSS,
        ] {
            collect_stock_horizontal_size_rules(css, &mut rules);
        }
        rules
    })
}

fn stock_selector_horizontal_size_rules() -> &'static HashMap<String, HashMap<String, String>> {
    static RULES: OnceLock<HashMap<String, HashMap<String, String>>> = OnceLock::new();
    RULES.get_or_init(|| {
        let mut rules = HashMap::new();
        for css in [
            DEFAULT_BASE_CSS,
            DEFAULT_PANEL_CSS,
            DEFAULT_POPUP_CSS,
            DEFAULT_WIDGETS_CSS,
            DEFAULT_MEDIA_CSS,
        ] {
            collect_stock_selector_horizontal_rules(css, &mut rules);
        }
        rules
    })
}

fn collect_stock_horizontal_size_rules(
    css: &'static str,
    rules: &mut HashMap<String, HashMap<String, String>>,
) {
    // Strip comments here too so the baseline cache sees the same tokens as the live geometry parser
    let stripped = strip_css_comments(css);
    let bytes = stripped.as_bytes();
    let mut cursor = 0usize;
    while let Some((selector, block, next)) = next_css_block(bytes, cursor) {
        cursor = next;
        let selector = normalize_selector(&selector);
        if selector.starts_with('@') {
            continue;
        }

        let declarations = parse_css_declarations(&block)
            .into_iter()
            .filter(|(name, _)| is_horizontal_size_property(name))
            .collect::<Vec<_>>();
        if declarations.is_empty() {
            continue;
        }

        for selector_part in split_selectors(&selector) {
            let trimmed = selector_part.trim();
            // Only single-class selectors map cleanly onto the lightweight width model
            if !trimmed.starts_with(".unixnotis-")
                || trimmed.contains(' ')
                || trimmed.contains('>')
                || trimmed.contains('+')
                || trimmed.contains('~')
                || trimmed.contains(':')
                || trimmed.contains('[')
                || trimmed.contains('#')
                || trimmed.contains(',')
                || trimmed.matches('.').count() != 1
            {
                continue;
            }

            let selector_rules = rules.entry(trimmed.to_string()).or_default();
            for (name, value) in &declarations {
                selector_rules.insert(name.trim().to_string(), value.trim().to_string());
            }
        }
    }
}

fn collect_stock_selector_horizontal_rules(
    css: &'static str,
    rules: &mut HashMap<String, HashMap<String, String>>,
) {
    // Complex selector matching uses the exact selector text instead of the single-class path
    let stripped = strip_css_comments(css);
    let bytes = stripped.as_bytes();
    let mut cursor = 0usize;
    while let Some((selector, block, next)) = next_css_block(bytes, cursor) {
        cursor = next;
        let selector = normalize_selector(&selector);
        if selector.starts_with('@') {
            continue;
        }

        let declarations = parse_css_declarations(&block)
            .into_iter()
            .filter(|(name, _)| is_horizontal_size_property(name))
            .collect::<Vec<_>>();
        if declarations.is_empty() {
            continue;
        }

        for selector_part in split_selectors(&selector) {
            let trimmed = selector_part.trim();
            if !trimmed.contains(".unixnotis-") {
                // Non-UnixNotis selectors are outside the checker contract here
                continue;
            }

            let selector_rules = rules.entry(trimmed.to_string()).or_default();
            for (name, value) in &declarations {
                selector_rules.insert(name.trim().to_string(), value.trim().to_string());
            }
        }
    }
}
