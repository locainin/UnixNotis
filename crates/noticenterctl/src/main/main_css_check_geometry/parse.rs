//! CSS parsing helpers for geometry lint

use std::collections::HashSet;

use super::super::main_css_check_parse::{
    next_css_block, normalize_selector, parse_css_declarations, should_recurse_at_rule,
    split_selectors, strip_css_comments,
};
use super::model::{is_tracked_class, GeometryModel, HorizontalEdges};
use super::stock::known_unixnotis_classes;

pub(super) fn collect_geometry_from_contents(
    contents: &str,
    model: &mut GeometryModel,
) -> Vec<String> {
    let mut warnings = Vec::new();
    let mut warned_classes = HashSet::new();

    // Strip comments first so selector scanning only sees live CSS
    let stripped = strip_css_comments(contents);
    // Walk the whole tree so nested @media and @layer rules still contribute width data
    collect_geometry_block(&stripped, model, &mut warnings, &mut warned_classes);
    warnings
}

fn collect_geometry_block(
    contents: &str,
    model: &mut GeometryModel,
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
                // Nested blocks still matter for final width math
                collect_geometry_block(&block, model, warnings, warned_classes);
            }
            continue;
        }

        for selector_part in split_selectors(&selector) {
            if selector_part.is_empty() {
                continue;
            }
            collect_geometry_selector(&selector_part, &block, model, warnings, warned_classes);
        }
    }
}

fn collect_geometry_selector(
    selector: &str,
    block: &str,
    model: &mut GeometryModel,
    warnings: &mut Vec<String>,
    warned_classes: &mut HashSet<String>,
) {
    // Parse declarations once so both warning checks and width updates use the same view
    let properties = css_properties(block);
    if properties.is_empty() {
        return;
    }

    let has_horizontal_size_rules = properties
        .iter()
        .any(|(name, _)| is_horizontal_size_property(name));
    let has_complex_width_driver_rules = properties
        .iter()
        .any(|(name, _)| is_complex_warning_property(name));
    // Keep selector matching strict so guessed width math does not drift from real widgets
    let Some(class_name) = simple_class_selector(selector) else {
        // Complex tracked selectors are better warned than silently skipped
        maybe_warn_for_complex_unixnotis_selector(
            selector,
            has_complex_width_driver_rules,
            warnings,
            warned_classes,
        );
        return;
    };

    if has_horizontal_size_rules
        && class_name.starts_with(".unixnotis-")
        && !known_unixnotis_classes().contains(class_name)
        && warned_classes.insert(class_name.to_string())
    {
        // Unknown class warnings are emitted once per file to stay readable
        warnings.push(format!(
            "size rules target unknown UnixNotis class '{}'; the live widget tree may never match it",
            class_name
        ));
    }

    let Some(target) = model.target_mut(class_name) else {
        // Unknown non-UnixNotis classes are ignored because the lint has no widget mapping for them
        return;
    };

    for (name, value) in properties {
        if is_horizontal_size_property(&name) {
            // Geometry lint only tracks properties that change horizontal width
            target.apply_property(&name, &value);
        }
    }
}

fn maybe_warn_for_complex_unixnotis_selector(
    selector: &str,
    has_horizontal_size_rules: bool,
    warnings: &mut Vec<String>,
    warned_classes: &mut HashSet<String>,
) {
    if !has_horizontal_size_rules
        || !is_compound_class_selector(selector)
        || !selector_mentions_tracked_class(selector)
    {
        // Descendant GTK node selectors are noisy here and do not map cleanly to width math
        return;
    }

    let warning_key = format!("complex:{selector}");
    if !warned_classes.insert(warning_key) {
        return;
    }

    warnings.push(format!(
        "size rules target complex UnixNotis selector '{}'; geometry lint only models single-class selectors, so width pressure may be missed",
        selector
    ));
}

fn is_compound_class_selector(selector: &str) -> bool {
    let trimmed = selector.trim();
    // Only same-element class chains are warned here
    trimmed.starts_with('.')
        && trimmed.matches('.').count() > 1
        && !trimmed.contains(' ')
        && !trimmed.contains('>')
        && !trimmed.contains('+')
        && !trimmed.contains('~')
        && !trimmed.contains(':')
        && !trimmed.contains('[')
        && !trimmed.contains('#')
        && !trimmed.contains(',')
}

fn selector_mentions_tracked_class(selector: &str) -> bool {
    let bytes = selector.as_bytes();
    let mut index = 0usize;
    while index < bytes.len() {
        if bytes[index] != b'.' {
            index += 1;
            continue;
        }

        let start = index;
        index += 1;
        while index < bytes.len() {
            let byte = bytes[index];
            // Class scanning stops at the first non class-name byte
            if byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_' {
                index += 1;
            } else {
                break;
            }
        }

        if index > start + 1 && is_tracked_class(&selector[start..index]) {
            // One tracked class is enough to make the selector relevant to geometry lint
            return true;
        }
    }

    false
}

fn css_properties(block: &str) -> Vec<(String, String)> {
    // Reuse the shared declaration parser so lint and geometry stay behavior-identical
    parse_css_declarations(block)
}

fn simple_class_selector(selector: &str) -> Option<&str> {
    let trimmed = selector.trim();
    if !trimmed.starts_with('.') {
        // Element names and IDs are outside the small class-based model used here
        return None;
    }
    if trimmed.contains(' ')
        || trimmed.contains('>')
        || trimmed.contains('+')
        || trimmed.contains('~')
        || trimmed.contains(':')
        || trimmed.contains('[')
        || trimmed.contains('#')
        || trimmed.contains(',')
    {
        // Descendant and pseudo selectors are skipped to keep matching conservative
        return None;
    }
    if trimmed.matches('.').count() != 1 {
        // Compound class chains are ambiguous for this lightweight model
        return None;
    }
    Some(trimmed)
}

fn is_horizontal_size_property(name: &str) -> bool {
    matches!(
        name.trim(),
        "width"
            | "min-width"
            | "margin"
            | "margin-left"
            | "margin-right"
            | "padding"
            | "padding-left"
            | "padding-right"
            | "border"
            | "border-width"
            | "border-left"
            | "border-left-width"
            | "border-right"
            | "border-right-width"
    )
}

fn is_complex_warning_property(name: &str) -> bool {
    // Border-only tweaks are common state styling and usually do not drive row width by themselves
    matches!(
        name.trim(),
        "width"
            | "min-width"
            | "margin"
            | "margin-left"
            | "margin-right"
            | "padding"
            | "padding-left"
            | "padding-right"
    )
}

pub(super) fn set_edge(edge: &mut f32, value: &str) {
    if let Some(parsed) = parse_single_length(value) {
        *edge = parsed;
    }
}

pub(super) fn parse_box_edges(value: &str) -> Option<HorizontalEdges> {
    // CSS shorthands map to left and right edges based on token count
    let values = parse_length_tokens(value);
    match values.as_slice() {
        [] => None,
        [all] => Some(HorizontalEdges {
            left: *all,
            right: *all,
        }),
        [vertical, horizontal] => {
            let _ = vertical;
            Some(HorizontalEdges {
                left: *horizontal,
                right: *horizontal,
            })
        }
        [_, right, _, left] => Some(HorizontalEdges {
            left: *left,
            right: *right,
        }),
        [_, right, left] => Some(HorizontalEdges {
            left: *left,
            right: *right,
        }),
        _ => None,
    }
}

fn parse_length_tokens(value: &str) -> Vec<f32> {
    // Four tokens are enough for the full CSS box shorthand
    value
        .split_whitespace()
        .filter_map(parse_length_token)
        .take(4)
        .collect()
}

pub(super) fn parse_single_length(value: &str) -> Option<f32> {
    // Use the first plain length token and ignore the rest
    value.split_whitespace().find_map(parse_length_token)
}

fn parse_length_token(token: &str) -> Option<f32> {
    let trimmed = token.trim().trim_end_matches(',');
    if trimmed.is_empty()
        || trimmed.contains('%')
        || trimmed.contains("calc(")
        || trimmed.contains("var(")
    {
        // Unsupported web-style values are ignored here and handled by parse hints elsewhere
        return None;
    }
    // GTK theme sizes in this codebase are plain px-like numbers, so keep parsing simple
    let numeric = trimmed
        .strip_suffix("px")
        .or_else(|| trimmed.strip_suffix("PX"))
        .unwrap_or(trimmed);
    numeric.parse::<f32>().ok()
}
