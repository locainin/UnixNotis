use super::lint_css_contents_with_properties;
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
