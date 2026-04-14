use super::super::super::main_css_check_parse::{
    next_css_block, normalize_selector, parse_css_declarations, should_recurse_at_rule,
};
use super::CssCustomProperties;

// Geometry only needs the last value for each custom property name
pub(super) fn collect_custom_properties(contents: &str) -> CssCustomProperties {
    let mut properties = CssCustomProperties::new();
    // One prepass is enough because geometry only needs the latest token values
    collect_custom_properties_block(contents, &mut properties);
    properties
}

fn collect_custom_properties_block(contents: &str, properties: &mut CssCustomProperties) {
    let mut cursor = 0usize;
    let bytes = contents.as_bytes();
    while let Some((selector, block, next)) = next_css_block(bytes, cursor) {
        cursor = next;
        let selector = normalize_selector(&selector);
        if selector.is_empty() {
            continue;
        }

        if selector.starts_with('@') {
            if should_recurse_at_rule(&selector) {
                // Nested at-rules still contribute custom properties to later selectors
                collect_custom_properties_block(&block, properties);
            }
            continue;
        }

        for (name, value) in parse_css_declarations(&block) {
            if name.trim_start().starts_with("--") {
                // Later declarations should override earlier ones like normal CSS cascade
                properties.insert(name.trim().to_string(), value.trim().to_string());
            }
        }
    }
}
