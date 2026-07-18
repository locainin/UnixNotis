use super::model::GeometryModel;
use super::parse::{
    collect_custom_property_scopes, collect_geometry_from_contents_with_properties,
};

pub(in crate::css_check::geometry) fn collect_geometry_from_contents(
    contents: &str,
    model: &mut GeometryModel,
) -> Vec<String> {
    // Test cases use the same property-aware parser as complete theme checks
    let properties = collect_custom_property_scopes(contents);
    collect_geometry_from_contents_with_properties(contents, &properties, model)
}
