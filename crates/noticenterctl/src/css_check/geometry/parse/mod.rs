//! CSS token, selector, custom-property, and geometry scanning

mod custom_properties;
mod lengths;
mod scan;
mod selectors;

pub(in crate::css_check) use custom_properties::CssCustomPropertyScopes;
pub(in crate::css_check::geometry) use lengths::{
    parse_box_edges, parse_box_vertical_edges, parse_single_length, set_edge,
};
pub(in crate::css_check::geometry) use scan::collect_geometry_from_contents_with_properties;
pub(in crate::css_check::geometry) use scan::CssCustomProperties;
pub(in crate::css_check) use scan::{
    can_model_horizontal_size_value, collect_custom_property_scopes,
};

#[cfg(test)]
#[path = "tests/scan.rs"]
mod tests;
