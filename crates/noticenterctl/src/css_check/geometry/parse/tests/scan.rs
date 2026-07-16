use super::selectors::{
    complex_target_class, is_nonexpanding_boundary_reset, simple_class_selector,
};
use super::{
    can_model_horizontal_size_value, collect_custom_property_scopes,
    collect_geometry_from_contents, CssCustomProperties,
};
use crate::css_check::geometry::model::GeometryModel;

#[test]
fn custom_property_scopes_apply_root_tokens_and_selector_overrides() {
    let css = r"
        :root { --gap: 10px; --shared: 12px; }
        .unixnotis-toggle { --gap: 18px; }
        .unixnotis-toggle:hover { --ignored: 99px; }
    ";

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
    let css = r"
        .unixnotis-made-up { min-width: 40px; }
        .unixnotis-made-up { padding-left: 4px; }
    ";
    let mut model = GeometryModel::default();

    let warnings = collect_geometry_from_contents(css, &mut model);

    assert_eq!(warnings.len(), 1);
    assert!(warnings[0].contains("unknown UnixNotis class '.unixnotis-made-up'"));
}

#[test]
fn geometry_collection_models_rightmost_unixnotis_descendant() {
    let css = ".unixnotis-panel .unixnotis-toggle { min-width: 40px; }";
    let mut model = GeometryModel::default();

    let warnings = collect_geometry_from_contents(css, &mut model);

    assert!(warnings.is_empty(), "{warnings:?}");
    let toggle = model
        .target_mut(".unixnotis-toggle")
        .expect("toggle target");
    assert_eq!(toggle.outer_width_px(0), 40);
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

#[test]
fn simple_class_selector_accepts_only_one_plain_class() {
    assert_eq!(
        simple_class_selector(" .unixnotis-toggle "),
        Some(".unixnotis-toggle")
    );
    assert_eq!(simple_class_selector("button"), None);
    assert_eq!(simple_class_selector(".unixnotis-toggle.active"), None);
    assert_eq!(simple_class_selector(".unixnotis-toggle:hover"), None);
    assert_eq!(
        simple_class_selector(".unixnotis-panel .unixnotis-toggle"),
        None
    );
    assert_eq!(
        simple_class_selector(".unixnotis-panel>.unixnotis-toggle"),
        None
    );
    assert_eq!(
        simple_class_selector(".unixnotis-panel+.unixnotis-toggle"),
        None
    );
    assert_eq!(
        simple_class_selector(".unixnotis-panel~.unixnotis-toggle"),
        None
    );
    assert_eq!(simple_class_selector(".unixnotis-toggle[disabled]"), None);
    assert_eq!(simple_class_selector(".unixnotis-toggle#primary"), None);
    assert_eq!(
        simple_class_selector(".unixnotis-toggle,.unixnotis-stat-card"),
        None
    );
    assert_eq!(simple_class_selector(".unixnotis toggle"), None);
    assert_eq!(simple_class_selector(".unixnotis>toggle"), None);
    assert_eq!(simple_class_selector(".unixnotis+toggle"), None);
    assert_eq!(simple_class_selector(".unixnotis~toggle"), None);
}

#[test]
fn complex_target_class_uses_the_rightmost_structural_hook() {
    assert_eq!(
        complex_target_class(".unixnotis-panel > .unixnotis-toggle.active:hover").as_deref(),
        Some(".unixnotis-toggle")
    );
    assert_eq!(
        complex_target_class(".unixnotis-panel button.flat").as_deref(),
        None
    );
    assert_eq!(complex_target_class("button.flat"), None);
    assert_eq!(
        complex_target_class(".unixnotis-panel+.unixnotis-toggle.active").as_deref(),
        Some(".unixnotis-toggle")
    );
    assert_eq!(
        complex_target_class(".unixnotis-panel~.unixnotis-stat-card:first-child").as_deref(),
        Some(".unixnotis-stat-card")
    );
}

#[test]
fn boundary_reset_requires_a_boundary_pseudo_class_and_zero_side_margins() {
    let zero_sides = vec![
        ("margin-left".to_string(), "0".to_string()),
        ("margin-right".to_string(), "0px".to_string()),
    ];
    let positive_side = vec![("margin-left".to_string(), "1px".to_string())];

    assert!(is_nonexpanding_boundary_reset(
        ".unixnotis-toggle:first-child",
        &zero_sides
    ));
    assert!(is_nonexpanding_boundary_reset(
        ".unixnotis-toggle:last-child",
        &zero_sides
    ));
    assert!(!is_nonexpanding_boundary_reset(
        ".unixnotis-toggle",
        &zero_sides
    ));
    assert!(!is_nonexpanding_boundary_reset(
        ".unixnotis-toggle:first-child",
        &positive_side
    ));
    assert!(!is_nonexpanding_boundary_reset(
        ".unixnotis-toggle:first-child",
        &[]
    ));
}

#[test]
fn geometry_collection_models_each_selector_in_a_selector_list() {
    let css = ".unixnotis-toggle, .unixnotis-stat-card { min-width: 48px; }";
    let mut model = GeometryModel::default();

    let warnings = collect_geometry_from_contents(css, &mut model);

    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(
        model
            .target_mut(".unixnotis-toggle")
            .expect("toggle target")
            .outer_width_px(0),
        48
    );
    assert_eq!(
        model
            .target_mut(".unixnotis-stat-card")
            .expect("stat target")
            .outer_width_px(0),
        48
    );
}

#[test]
fn geometry_collection_ignores_non_recursive_at_rules() {
    let css = "@keyframes pulse { from { width: 500px; } to { width: 900px; } }";
    let mut model = GeometryModel::default();

    let warnings = collect_geometry_from_contents(css, &mut model);

    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(
        model
            .target_mut(".unixnotis-panel")
            .expect("panel target")
            .outer_width_px(0),
        0
    );
}

#[test]
fn compound_selector_uses_target_scoped_custom_properties() {
    let css = r"
        :root { --item-width: 20px; }
        .unixnotis-toggle { --item-width: 44px; }
        .unixnotis-toggle.active { min-width: var(--item-width); }
    ";
    let mut model = GeometryModel::default();

    let warnings = collect_geometry_from_contents(css, &mut model);

    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(
        model
            .target_mut(".unixnotis-toggle")
            .expect("toggle target")
            .outer_width_px(0),
        44
    );
}
