use super::*;

#[test]
fn merge_css_with_overrides_appends_overrides_to_untouched_defaults() {
    let fallback = ".panel { color: red; }";
    let overrides = ".panel { color: blue; }";

    let merged = merge_css_with_overrides(fallback, fallback, overrides);

    assert_eq!(merged, ".panel { color: red; }\n.panel { color: blue; }");
}

#[test]
fn merge_css_with_overrides_prepends_overrides_before_user_edited_css() {
    let fallback = ".panel { color: red; }";
    let user_css = ".panel { color: green; }";
    let overrides = ".panel { color: blue; }";

    let merged = merge_css_with_overrides(user_css, fallback, overrides);

    assert_eq!(merged, ".panel { color: blue; }\n.panel { color: green; }");
}

#[test]
fn merge_css_with_overrides_leaves_contents_unchanged_when_overrides_are_empty() {
    let user_css = ".panel { color: green; }";

    let merged = merge_css_with_overrides(user_css, ".panel { color: red; }", "  ");

    assert_eq!(merged, user_css);
}
