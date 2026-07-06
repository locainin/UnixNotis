use super::{
    can_model_horizontal_size_value, collect_custom_property_scopes,
    collect_geometry_from_contents, CssCustomProperties,
};
use crate::css_check::geometry::model::GeometryModel;

#[test]
fn custom_property_scopes_apply_root_tokens_and_selector_overrides() {
    let css = r#"
        :root { --gap: 10px; --shared: 12px; }
        .unixnotis-toggle { --gap: 18px; }
        .unixnotis-toggle:hover { --ignored: 99px; }
    "#;

    let scopes = collect_custom_property_scopes(css);

    let toggle = scopes.for_selector(".unixnotis-toggle");
    assert_eq!(toggle.get("--gap").map(String::as_str), Some("18px"));
    assert_eq!(toggle.get("--shared").map(String::as_str), Some("12px"));
    assert!(!toggle.contains_key("--ignored"));

    let panel = scopes.for_selector(".unixnotis-panel");
    assert_eq!(panel.get("--gap").map(String::as_str), Some("10px"));
}

#[test]
fn can_model_horizontal_size_value_rejects_unresolved_width_tokens() {
    let css = ":root { --known: calc(10px + 2px); }";
    let scopes = collect_custom_property_scopes(css);

    assert!(can_model_horizontal_size_value(
        ".unixnotis-toggle",
        "min-width",
        "var(--known)",
        &scopes
    ));
    assert!(!can_model_horizontal_size_value(
        ".unixnotis-toggle",
        "min-width",
        "var(--missing)",
        &scopes
    ));
    assert!(can_model_horizontal_size_value(
        ".unixnotis-toggle",
        "color",
        "var(--missing)",
        &scopes
    ));
}

#[test]
fn geometry_collection_warns_once_for_unknown_unixnotis_size_class() {
    let css = r#"
        .unixnotis-made-up { min-width: 40px; }
        .unixnotis-made-up { padding-left: 4px; }
    "#;
    let mut model = GeometryModel::default();

    let warnings = collect_geometry_from_contents(css, &mut model);

    assert_eq!(warnings.len(), 1);
    assert!(warnings[0].contains("unknown UnixNotis class '.unixnotis-made-up'"));
}

#[test]
fn geometry_collection_warns_for_complex_unmodeled_unixnotis_width_selector() {
    let css = ".unixnotis-panel .unixnotis-toggle { min-width: 40px; }";
    let mut model = GeometryModel::default();

    let warnings = collect_geometry_from_contents(css, &mut model);

    assert_eq!(warnings.len(), 1);
    assert!(warnings[0].contains("complex UnixNotis selector"));
}

#[test]
fn geometry_collection_applies_nested_media_rules_to_width_model() {
    let css = "@media (min-width: 1px) { .unixnotis-panel { padding: 11px; } }";
    let mut model = GeometryModel::default();

    let warnings = collect_geometry_from_contents(css, &mut model);

    assert!(warnings.is_empty(), "{warnings:?}");
    let panel = model.target_mut(".unixnotis-panel").expect("panel target");
    assert_eq!(panel.inner_insets_px(), 22);
}

#[test]
fn plain_custom_properties_type_accepts_direct_lengths() {
    let mut properties = CssCustomProperties::new();
    properties.insert("--w".to_string(), "42px".to_string());

    assert_eq!(
        super::parse_single_length("var(--w)", &properties),
        Some(42.0)
    );
}
