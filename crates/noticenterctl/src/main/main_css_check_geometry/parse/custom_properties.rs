use std::collections::HashMap;

use super::super::super::main_css_check_parse::{
    next_css_block, normalize_selector, parse_css_declarations, should_recurse_at_rule,
    split_selectors,
};
use super::selectors::simple_class_selector;
use super::CssCustomProperties;

pub(in crate::main_css_check) struct CssCustomPropertyScopes {
    // Root tokens apply everywhere unless a tracked selector overrides them later
    root: CssCustomProperties,
    // Selector scopes only keep simple class selectors that geometry can reason about
    selectors: HashMap<String, CssCustomProperties>,
}

impl CssCustomPropertyScopes {
    pub(in crate::main_css_check) fn for_selector(&self, selector: &str) -> CssCustomProperties {
        let mut resolved = self.root.clone();
        if let Some(selector_scope) = self.selectors.get(selector) {
            // Selector-local tokens override root tokens for that widget class
            resolved.extend(selector_scope.clone());
        }
        resolved
    }
}

// Geometry only needs the latest custom property values that can reach tracked widgets
pub(super) fn collect_custom_properties(contents: &str) -> CssCustomPropertyScopes {
    let mut scopes = CssCustomPropertyScopes {
        root: CssCustomProperties::new(),
        selectors: HashMap::new(),
    };
    // One prepass is enough because computed custom properties are last-write-wins
    collect_custom_properties_block(contents, &mut scopes);
    scopes
}

fn collect_custom_properties_block(contents: &str, scopes: &mut CssCustomPropertyScopes) {
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
                collect_custom_properties_block(&block, scopes);
            }
            continue;
        }

        // Only custom property declarations matter in this prepass
        let declarations = parse_css_declarations(&block)
            .into_iter()
            .filter(|(name, _)| name.trim_start().starts_with("--"))
            .collect::<Vec<_>>();
        if declarations.is_empty() {
            continue;
        }

        for selector_part in split_selectors(&selector) {
            if selector_part.is_empty() {
                continue;
            }

            let trimmed = selector_part.trim();
            if trimmed == ":root" {
                for (name, value) in &declarations {
                    // Root tokens apply to every widget unless a later selector overrides them
                    scopes
                        .root
                        .insert(name.trim().to_string(), value.trim().to_string());
                }
                continue;
            }

            let Some(class_name) = simple_class_selector(trimmed) else {
                // Complex selector-scoped custom properties are skipped to keep the model conservative
                continue;
            };

            let selector_scope = scopes.selectors.entry(class_name.to_string()).or_default();
            for (name, value) in &declarations {
                selector_scope.insert(name.trim().to_string(), value.trim().to_string());
            }
        }
    }
}
