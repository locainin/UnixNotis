use std::collections::HashMap;
use std::sync::OnceLock;

use unixnotis_core::{
    build_modern_theme_custom_properties, gtk_css_features_for_version, Config, DEFAULT_BASE_CSS,
    DEFAULT_MEDIA_CSS, DEFAULT_PANEL_CSS, DEFAULT_POPUP_CSS, DEFAULT_WIDGETS_CSS,
};

use super::super::super::main_css_check_parse::{
    next_css_block, normalize_selector, parse_css_declarations, split_selectors, strip_css_comments,
};
use super::super::super::main_css_check_policy::is_horizontal_size_property;
use super::super::model::GeometryModel;
use super::super::parse::{
    collect_custom_property_scopes, collect_geometry_from_contents_with_properties,
};
use super::normalized_horizontal_size_rules;

pub(in crate::main_css_check) fn stock_matches_complex_selector_rules(
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

pub(in crate::main_css_check) fn stock_config() -> &'static Config {
    static CONFIG: OnceLock<Config> = OnceLock::new();
    // Default config is a stable baseline for false-positive control
    CONFIG.get_or_init(Config::default)
}

pub(in crate::main_css_check) fn stock_geometry_model() -> &'static GeometryModel {
    static MODEL: OnceLock<GeometryModel> = OnceLock::new();
    MODEL.get_or_init(|| {
        let mut model = GeometryModel::default();
        let generated_tokens = build_modern_theme_custom_properties(
            &stock_config().theme,
            gtk_css_features_for_version(4, 16),
        );
        let shared_custom_properties = collect_custom_property_scopes(
            &std::iter::once(generated_tokens.as_str())
                .chain([
                    DEFAULT_BASE_CSS,
                    DEFAULT_PANEL_CSS,
                    DEFAULT_POPUP_CSS,
                    DEFAULT_WIDGETS_CSS,
                    DEFAULT_MEDIA_CSS,
                ])
                .collect::<Vec<_>>()
                .join("\n"),
        );

        // Merge every shipped CSS file into one baseline geometry model
        for css in [
            DEFAULT_BASE_CSS,
            DEFAULT_PANEL_CSS,
            DEFAULT_POPUP_CSS,
            DEFAULT_WIDGETS_CSS,
            DEFAULT_MEDIA_CSS,
        ] {
            // The shipped theme is the baseline used to keep false positives low
            let _ = collect_geometry_from_contents_with_properties(
                css,
                &shared_custom_properties,
                &mut model,
            );
        }
        model
    })
}

pub(super) fn stock_horizontal_size_rules() -> &'static HashMap<String, HashMap<String, String>> {
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
