use super::lint_css_contents_with_properties;
use crate::css_check::geometry::collect_custom_property_scopes;
use crate::css_check::lint::test_support::lint_css_contents;

#[test]
fn duplicate_selector_warns_inside_same_at_rule_context() {
    let css = "@media (min-width: 1px) { .a { color: red; } .a { color: blue; } }";

    let warnings = lint_css_contents(css);

    assert_eq!(warnings.len(), 1);
    assert!(warnings[0].message.contains("duplicate selector '.a'"));
    assert!(warnings[0]
        .message
        .contains("within @media (min-width: 1px)"));
}

#[test]
fn duplicate_selector_in_different_at_rule_contexts_stays_quiet() {
    let css = r"
        @media (min-width: 1px) { .a { color: red; } }
        @media (min-width: 2px) { .a { color: blue; } }
    ";

    let warnings = lint_css_contents(css);

    assert!(warnings
        .iter()
        .all(|warning| !warning.message.contains("duplicate selector")));
}

#[test]
fn duplicate_property_suppresses_identical_value_and_resolved_modern_fallback() {
    let css = r"
        :root { --wide: 144px; }
        .unixnotis-toggle {
            color: red;
            color: red;
            min-width: 120px;
            min-width: var(--wide);
        }
    ";
    let custom_properties = collect_custom_property_scopes(css);

    let warnings = lint_css_contents_with_properties(css, &custom_properties);

    assert!(warnings.is_empty(), "{warnings:?}");
}

#[test]
fn duplicate_property_warns_when_modern_width_fallback_cannot_resolve() {
    let css = r"
        .unixnotis-toggle {
            min-width: 120px;
            min-width: var(--missing-width);
        }
    ";
    let custom_properties = collect_custom_property_scopes(css);

    let warnings = lint_css_contents_with_properties(css, &custom_properties);

    assert!(warnings
        .iter()
        .any(|warning| warning.message.contains("duplicate property 'min-width'")));
    assert!(warnings
        .iter()
        .any(|warning| warning.message.contains("uses var() in a layout value")));
}

#[test]
fn width_values_warn_for_percentage_non_px_and_unresolved_compare_math() {
    let css = r"
        .unixnotis-panel { min-width: 80%; }
        .unixnotis-toggle { min-width: 12rem; }
        .unixnotis-stat-card { min-width: max(10px, var(--missing)); }
    ";

    let messages = lint_css_contents(css)
        .into_iter()
        .map(|warning| warning.message)
        .collect::<Vec<_>>();

    assert!(messages
        .iter()
        .any(|message| message.contains("percentage lengths")));
    assert!(messages
        .iter()
        .any(|message| message.contains("non-px length units")));
    assert!(messages
        .iter()
        .any(|message| message.contains("min(), max(), or clamp()")));
}
