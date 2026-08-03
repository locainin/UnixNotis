use super::{lint_css_contents_with_options, lint_css_contents_with_properties, LintOptions};
use crate::css_check::geometry::collect_custom_property_scopes;

#[test]
fn scanner_reports_duplicate_selectors_with_source_location() {
    let css = ".item { color: red; }\n.item { color: blue; }";
    let properties = collect_custom_property_scopes(css);

    let findings = lint_css_contents_with_properties(css, &properties);
    let duplicate = findings
        .iter()
        .find(|finding| finding.message.contains("duplicate selector"))
        .expect("duplicate selector should be reported");

    assert_eq!(duplicate.line, Some(2));
    assert!(duplicate.column.is_some());
}

#[test]
fn nested_duplicate_selector_reports_its_absolute_source_location() {
    let css = "@media (min-width: 1px) {\n  .item { color: red; }\n  .item { color: blue; }\n}";
    let properties = collect_custom_property_scopes(css);

    let findings = lint_css_contents_with_properties(css, &properties);
    let duplicate = findings
        .iter()
        .find(|finding| finding.message.contains("duplicate selector"))
        .expect("nested duplicate selector should be reported");

    assert_eq!((duplicate.line, duplicate.column), (Some(3), Some(3)));
}

#[test]
fn grouped_duplicate_selector_reports_the_repeated_member_location() {
    let css = ".a, .b { color: red; }\n.x, .b { color: blue; }";
    let properties = collect_custom_property_scopes(css);

    let findings = lint_css_contents_with_properties(css, &properties);
    let duplicate = findings
        .iter()
        .find(|finding| finding.message.contains("duplicate selector '.b'"))
        .expect("grouped duplicate selector should be reported");

    assert_eq!((duplicate.line, duplicate.column), (Some(2), Some(5)));
}

#[test]
fn nested_duplicate_property_reports_its_absolute_source_location() {
    let css = "@media (min-width: 1px) {\n  .item {\n    color: red;\n    color: blue;\n  }\n}";
    let properties = collect_custom_property_scopes(css);

    let findings = lint_css_contents_with_properties(css, &properties);
    let duplicate = findings
        .iter()
        .find(|finding| finding.message.contains("duplicate property 'color'"))
        .expect("nested duplicate property should be reported");

    assert_eq!((duplicate.line, duplicate.column), (Some(4), Some(5)));
}

#[test]
fn scanner_suppresses_only_duplicates_inside_a_closed_override_section() {
    let css = "
        .item { color: red; }
        /* unixnotis-css-check allow-duplicate-selectors:start */
        .item { color: blue; }
        /* unixnotis-css-check allow-duplicate-selectors:end */
        .item { color: green; }
    ";
    let properties = collect_custom_property_scopes(css);

    let findings = lint_css_contents_with_properties(css, &properties);
    let duplicates = findings
        .iter()
        .filter(|finding| finding.message.contains("duplicate selector"))
        .collect::<Vec<_>>();

    assert_eq!(duplicates.len(), 1);
    assert_eq!(duplicates[0].line, Some(6));
}

#[test]
fn shipped_css_assets_are_lint_clean() {
    let assets = [
        unixnotis_core::DEFAULT_BASE_CSS,
        unixnotis_core::DEFAULT_PANEL_CSS,
        unixnotis_core::DEFAULT_POPUP_CSS,
        unixnotis_core::DEFAULT_WIDGETS_CSS,
        unixnotis_core::DEFAULT_MEDIA_CSS,
    ];
    let config = unixnotis_core::Config::default();
    let generated = unixnotis_core::build_modern_theme_custom_properties(&config.theme);
    let combined = std::iter::once(generated.as_str())
        .chain(assets)
        .collect::<Vec<_>>()
        .join("\n");
    let properties = collect_custom_property_scopes(&combined);

    for css in assets {
        let findings = lint_css_contents_with_options(
            css,
            &properties,
            LintOptions {
                honor_suppressions: false,
            },
        );

        assert!(findings.is_empty(), "{findings:?}");
    }
}

#[test]
fn current_stock_assets_contain_no_lint_suppressions() {
    for css in [
        unixnotis_core::DEFAULT_BASE_CSS,
        unixnotis_core::DEFAULT_PANEL_CSS,
        unixnotis_core::DEFAULT_POPUP_CSS,
        unixnotis_core::DEFAULT_WIDGETS_CSS,
        unixnotis_core::DEFAULT_MEDIA_CSS,
    ] {
        assert!(
            !css.contains("unixnotis-css-check allow-duplicate-selectors"),
            "current stock CSS must not suppress repository lint findings"
        );
    }
}
