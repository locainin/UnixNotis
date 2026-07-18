use super::runner::lint_css_contents_with_properties;
use crate::css_check::geometry::collect_custom_property_scopes;
use crate::css_check::lint::CssCheckLintFinding;

pub(in crate::css_check) fn lint_css_contents(contents: &str) -> Vec<CssCheckLintFinding> {
    // Unit cases share the custom-property collection used by file-based linting
    lint_css_contents_with_properties(contents, &collect_custom_property_scopes(contents))
}
