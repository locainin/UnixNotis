//! Geometry declaration and selector scanning

use std::collections::{HashMap, HashSet};

use super::super::super::parse::{
    next_css_block, normalize_selector, parse_css_declarations, should_recurse_at_rule,
    split_selectors, strip_css_comments,
};
use super::super::super::policy::{is_horizontal_size_property, is_vertical_size_property};
use super::super::model::GeometryModel;
use super::super::stock::baselines::stock_matches_complex_selector_rules;
use super::super::stock::classes::is_known_unixnotis_class;
use super::super::stock::should_warn_for_unmodeled_known_class;

pub(in crate::css_check::geometry) type CssCustomProperties = HashMap<String, String>;

use super::custom_properties::collect_custom_properties;
use super::custom_properties::CssCustomPropertyScopes;
use super::lengths::{parse_box_edges, parse_single_length};
use super::selectors::{
    complex_target_class, is_nonexpanding_boundary_reset, simple_class_selector,
};

#[cfg(test)]
pub(in crate::css_check::geometry) fn collect_geometry_from_contents(
    contents: &str,
    model: &mut GeometryModel,
) -> Vec<String> {
    // Single-file callers still get the old behavior through this small wrapper
    // This path is still used by unit tests and by stock-baseline helpers
    let stripped = strip_css_comments(contents);
    let custom_properties = collect_custom_properties(&stripped);
    collect_geometry_from_contents_with_properties(&stripped, &custom_properties, model)
}

pub(in crate::css_check::geometry) fn collect_geometry_from_contents_with_properties(
    contents: &str,
    custom_properties: &CssCustomPropertyScopes,
    model: &mut GeometryModel,
) -> Vec<String> {
    let mut warnings = Vec::new();
    let mut warned_classes = HashSet::new();
    let stripped_contents = strip_css_comments(contents);

    // Comments are removed before both selector walking and custom-property resolution so the
    // parser and the stock-baseline cache see the same token stream
    // Walk the full sheet so nested blocks still contribute width data
    collect_geometry_block(
        &stripped_contents,
        model,
        custom_properties,
        &mut warnings,
        &mut warned_classes,
    );
    warnings
}

pub(in crate::css_check) fn collect_custom_property_scopes(
    contents: &str,
) -> CssCustomPropertyScopes {
    // Lint and geometry both need the same custom-property view of the file
    collect_custom_properties(contents)
}

pub(in crate::css_check) fn can_model_horizontal_size_value(
    selector: &str,
    property: &str,
    value: &str,
    custom_properties: &CssCustomPropertyScopes,
) -> bool {
    // Geometry only understands tracked width-driving properties
    if !is_horizontal_size_property(property) {
        return true;
    }

    let modeled_class = simple_class_selector(selector)
        .map(str::to_string)
        .or_else(|| complex_target_class(selector));
    let scoped_properties = modeled_class.as_deref().map_or_else(
        || custom_properties.for_selector(selector),
        |class_name| custom_properties.for_selector(class_name),
    );

    match property {
        "width" | "min-width" | "margin-left" | "margin-right" | "padding-left"
        | "padding-right" | "border-left" | "border-left-width" | "border-right"
        | "border-right-width" => parse_single_length(value, &scoped_properties).is_some(),
        "margin" | "padding" | "border" | "border-width" => {
            parse_box_edges(value, &scoped_properties).is_some()
        }
        _ => false,
    }
}

fn collect_geometry_block(
    contents: &str,
    model: &mut GeometryModel,
    custom_properties: &CssCustomPropertyScopes,
    warnings: &mut Vec<String>,
    warned_classes: &mut HashSet<String>,
) {
    let mut cursor = 0usize;
    let bytes = contents.as_bytes();
    while let Some((selector, block, next)) = next_css_block(bytes, cursor) {
        cursor = next;
        // Normalization keeps selector matching stable across spacing styles
        let selector = normalize_selector(&selector);
        if selector.is_empty() {
            continue;
        }

        if selector.starts_with('@') {
            if should_recurse_at_rule(&selector) {
                // Width-relevant selectors can live under nested rules, so the block still gets
                // walked even though the at-rule itself is not modeled directly
                // Nested blocks still matter for final width math
                collect_geometry_block(&block, model, custom_properties, warnings, warned_classes);
            }
            continue;
        }

        for selector_part in split_selectors(&selector) {
            if selector_part.is_empty() {
                continue;
            }
            collect_geometry_selector(
                &selector_part,
                &block,
                model,
                custom_properties,
                warnings,
                warned_classes,
            );
        }
    }
}

fn collect_geometry_selector(
    selector: &str,
    block: &str,
    model: &mut GeometryModel,
    custom_properties: &CssCustomPropertyScopes,
    warnings: &mut Vec<String>,
    warned_classes: &mut HashSet<String>,
) {
    // Parse declarations once so warnings and width updates see the same data
    let properties = css_properties(block);
    if properties.is_empty() {
        return;
    }

    let has_horizontal_size_rules = properties
        .iter()
        .any(|(name, _)| is_horizontal_size_property(name));
    let has_vertical_size_rules = properties
        .iter()
        .any(|(name, _)| is_vertical_size_property(name));
    // Keep selector matching strict so width math does not drift from real widgets
    let modeled_class = simple_class_selector(selector)
        .map(str::to_string)
        .or_else(|| complex_target_class(selector));
    let Some(class_name) = modeled_class.as_deref() else {
        // Descendants aimed at plain GTK subnodes do not resize a UnixNotis-owned allocation
        return;
    };

    if has_horizontal_size_rules
        && class_name.starts_with(".unixnotis-")
        && !is_known_unixnotis_class(class_name)
        && warned_classes.insert(class_name.to_string())
    {
        // Unknown class warnings are emitted once per file so output stays readable
        warnings.push(format!(
            "size rules target unknown UnixNotis class '{class_name}'; the live widget tree may never match it"
        ));
    }

    if has_horizontal_size_rules {
        let Some(target) = model.target_mut(class_name) else {
            if is_nonexpanding_boundary_reset(selector, &properties) {
                // Removing an outer margin cannot increase the widget width budget
                return;
            }
            if stock_matches_complex_selector_rules(selector, &properties) {
                // Shipped pseudo and descendant rules already have a reviewed geometry baseline
                return;
            }
            if has_horizontal_size_rules
                && class_name.starts_with(".unixnotis-")
                && is_known_unixnotis_class(class_name)
                && should_warn_for_unmodeled_known_class(class_name, &properties)
                && warned_classes.insert(format!("unmodeled:{class_name}"))
            {
                // The class is real, but there is no direct width math for it yet
                // Once the rules differ from stock, the change needs to stay visible
                // Only custom size changes on major unmodeled layout hooks should stay loud
                warnings.push(format!(
                    "size rules target known UnixNotis class '{class_name}', but geometry lint does not model its width yet; width pressure may be missed"
                ));
            }
            // Known non-modeled classes otherwise stay quiet so stock theme selectors do not spam output
            return;
        };
        let custom_properties = custom_properties.for_selector(class_name);

        for (name, value) in &properties {
            if is_horizontal_size_property(name) {
                // Every supported property is reduced into one horizontal box model so the final
                // budget check only has to compare plain numbers
                // Geometry lint only tracks properties that change horizontal width
                target.apply_property(name, value, &custom_properties);
            }
        }
    }

    if !has_vertical_size_rules {
        return;
    }

    let Some(target) = model.target_vertical_mut(class_name) else {
        return;
    };
    let custom_properties = custom_properties.for_selector(class_name);

    for (name, value) in properties {
        if is_vertical_size_property(&name) {
            // Media height warnings need top and bottom box math without widening the existing width path
            target.apply_property(&name, &value, &custom_properties);
        }
    }
}

fn css_properties(block: &str) -> Vec<(String, String)> {
    // Reuse the shared declaration parser so lint and geometry stay behavior-identical
    parse_css_declarations(block)
}
