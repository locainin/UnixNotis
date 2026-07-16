//! Geometry-aware CSS linting, parsing, and panel budget modeling

mod check;
mod model;
mod parse;
mod stock;
#[cfg(test)]
mod tests;

pub(in crate::css_check) use check::lint_geometry_css_files_with_config;
#[cfg(test)]
pub(in crate::css_check::geometry) use model::GeometryModel;
pub(super) use parse::{
    can_model_horizontal_size_value, collect_custom_property_scopes, CssCustomPropertyScopes,
};
