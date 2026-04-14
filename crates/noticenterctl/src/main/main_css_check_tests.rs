use super::main_css_check_lint::lint_css_contents;
use super::main_css_check_parse::{parse_css_declarations, split_selectors};
use super::main_css_check_runtime::panel_width_floor_warning;
use unixnotis_core::{
    build_modern_theme_custom_properties, gtk_css_features_for_version, Config, ThemeConfig,
    PANEL_RUNTIME_WIDTH_MIN,
};

#[test]
fn split_selectors_handles_is_commas() {
    // Commas inside :is() stay inside the selector
    let selector = ".a:is(.b, .c), .d";
    let parts = split_selectors(selector);
    assert_eq!(parts.len(), 2);
    assert_eq!(parts[0], ".a:is(.b, .c)");
    assert_eq!(parts[1], ".d");
}

#[test]
fn lint_css_contents_scans_media_blocks() {
    // Nested media rules still report duplicate selectors
    let css = "@media (min-width: 1px) { .a { color: red; } .a { color: blue; } }";
    let warnings = lint_css_contents(css);
    assert_eq!(warnings.len(), 1);
    assert!(warnings[0].message.contains("duplicate selector '.a'"));
    assert!(warnings[0]
        .message
        .contains("within @media (min-width: 1px)"));
    assert_eq!(warnings[0].line, Some(1));
    assert!(warnings[0].column.is_some());
}

#[test]
fn lint_css_contents_scans_layer_blocks() {
    // Layer blocks use the same duplicate selector rule
    let css = "@layer theme { .a { color: red; } .a { color: blue; } }";
    let warnings = lint_css_contents(css);
    assert_eq!(warnings.len(), 1);
    assert!(warnings[0].message.contains("duplicate selector '.a'"));
    assert!(warnings[0].message.contains("within @layer theme"));
}

#[test]
fn panel_width_floor_warning_reports_runtime_clamp() {
    // Width below the runtime floor should explain why the panel can still look fat
    let mut config = Config::default();
    config.panel.width = PANEL_RUNTIME_WIDTH_MIN - 1;

    let warning = panel_width_floor_warning(&config).expect("warning expected");
    assert!(warning.contains("[panel].width"));
    assert!(warning.contains("runtime floor"));
    assert!(warning.contains("panel may look wider"));
}

#[test]
fn lint_css_contents_warns_on_web_length_tokens_in_layout_props() {
    // Valid calc and var usage should pass when the token chain resolves cleanly
    let css = r#"
        :root {
            --pad: 12px;
        }
        .unixnotis-panel {
            min-width: calc(30px + 4px);
            padding-left: var(--pad);
        }
    "#;

    let warnings = lint_css_contents(css);
    assert!(warnings.is_empty());
}

#[test]
fn lint_css_contents_accepts_generated_modern_theme_tokens() {
    let css = format!(
        "{}\n.unixnotis-panel-card {{ border-radius: var(--unixnotis-card-radius); padding: calc(var(--unixnotis-panel-card-padding-y) + 2px) var(--unixnotis-panel-card-padding-x); }}",
        build_modern_theme_custom_properties(
            &ThemeConfig::default(),
            gtk_css_features_for_version(4, 16),
        )
    );

    let warnings = lint_css_contents(&css);
    assert!(warnings.is_empty(), "{warnings:?}");
}

#[test]
fn lint_css_contents_still_warns_on_percentage_layout_values() {
    // Percentages are still hard for geometry lint to model accurately
    let css = r#"
        .unixnotis-panel {
            min-width: 80%;
        }
    "#;

    let warnings = lint_css_contents(css);
    assert_eq!(warnings.len(), 1);
    assert!(warnings[0]
        .message
        .contains("geometry estimates may be incomplete"));
    assert!(warnings[0].line.is_some());
    assert!(warnings[0].column.is_some());
}

#[test]
fn lint_css_contents_reports_line_for_duplicate_property() {
    // Duplicate properties should point at the later property that wins
    let css = r#"
        .unixnotis-panel {
            padding: 6px;
            padding: 8px;
        }
    "#;

    let warnings = lint_css_contents(css);
    let duplicate = warnings
        .iter()
        .find(|warning| warning.code == "LINT003")
        .expect("duplicate property warning");
    assert_eq!(duplicate.line, Some(4));
    assert!(duplicate.column.is_some());
}

#[test]
fn panel_width_floor_warning_skips_safe_widths() {
    // Width at or above the runtime floor should stay quiet
    let mut config = Config::default();
    config.panel.width = PANEL_RUNTIME_WIDTH_MIN;

    assert!(panel_width_floor_warning(&config).is_none());
}

#[test]
fn parse_css_declarations_keeps_semicolons_inside_quoted_values() {
    let block = "background-image: url(\"data:image/svg+xml;utf8,<svg></svg>\"); color: red;";
    let declarations = parse_css_declarations(block);

    assert_eq!(declarations.len(), 2);
    assert_eq!(declarations[0].0, "background-image");
    assert!(declarations[0].1.contains("data:image/svg+xml;utf8"));
    assert_eq!(declarations[1], ("color".to_string(), "red".to_string()));
}
