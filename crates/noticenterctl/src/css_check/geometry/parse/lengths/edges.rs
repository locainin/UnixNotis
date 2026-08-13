//! CSS edge shorthand and single-length entry points

use super::super::super::model::{HorizontalEdges, VerticalEdges};
use super::tokenize::split_css_value_tokens;
use super::{parse_length_expression, CssCustomProperties, ResolvedCssValue};

// Length parsing stays local to the geometry parser so calc and var rules do not leak outward
pub(in crate::css_check::geometry) fn set_edge(
    edge: &mut f32,
    value: &str,
    custom_properties: &CssCustomProperties,
) {
    if let Some(parsed) = parse_single_length(value, custom_properties) {
        *edge = parsed;
    }
}

pub(in crate::css_check::geometry) fn parse_box_edges(
    value: &str,
    custom_properties: &CssCustomProperties,
) -> Option<HorizontalEdges> {
    // CSS shorthands map to left and right edges based on token count
    let values = parse_length_tokens(value, custom_properties);
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
        [_, right, _] => Some(HorizontalEdges {
            left: *right,
            right: *right,
        }),
        _ => None,
    }
}

pub(in crate::css_check::geometry) fn parse_box_vertical_edges(
    value: &str,
    custom_properties: &CssCustomProperties,
) -> Option<VerticalEdges> {
    // CSS shorthands map to top and bottom edges based on token count
    let values = parse_length_tokens(value, custom_properties);
    match values.as_slice() {
        [] => None,
        [all] => Some(VerticalEdges {
            top: *all,
            bottom: *all,
        }),
        [vertical, _horizontal] => Some(VerticalEdges {
            top: *vertical,
            bottom: *vertical,
        }),
        [top, _horizontal, bottom] => Some(VerticalEdges {
            top: *top,
            bottom: *bottom,
        }),
        [top, _, bottom, _left] => Some(VerticalEdges {
            top: *top,
            bottom: *bottom,
        }),
        _ => None,
    }
}

pub(in crate::css_check::geometry) fn parse_single_length(
    value: &str,
    custom_properties: &CssCustomProperties,
) -> Option<f32> {
    let trimmed = value.trim();
    if let Some(parsed) = parse_length_expression(trimmed, custom_properties, 0) {
        return parsed.into_length();
    }

    // Fall back to the first token so old shorthand behavior stays intact
    split_css_value_tokens(trimmed)
        .ok()?
        .into_iter()
        .find_map(|token| parse_length_expression(token, custom_properties, 0))
        .and_then(ResolvedCssValue::into_length)
}

fn parse_length_tokens(value: &str, custom_properties: &CssCustomProperties) -> Vec<f32> {
    // Four tokens are enough for the full CSS box shorthand
    split_css_value_tokens(value)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|token| parse_length_expression(token, custom_properties, 0))
        .filter_map(ResolvedCssValue::into_length)
        .take(4)
        .collect()
}
