use super::{
    DuplicateSelectorAllowlist, ALLOW_DUPLICATE_SELECTORS_END, ALLOW_DUPLICATE_SELECTORS_START,
};

#[test]
fn closed_duplicate_selector_directive_only_allows_its_own_range() {
    let long_prefix = "x".repeat(ALLOW_DUPLICATE_SELECTORS_START.len() + 16);
    let source = format!(
        "{long_prefix}\n.before-marker {{}}\n\
         {ALLOW_DUPLICATE_SELECTORS_START}\n.inside {{}}\n\
         {ALLOW_DUPLICATE_SELECTORS_END}\n.after {{}}"
    );
    let allowlist = DuplicateSelectorAllowlist::from_source(&source);

    assert!(!allowlist.contains(
        source
            .find(".before-marker")
            .expect("selector before marker")
    ));
    assert!(allowlist.contains(source.find(".inside").expect("inside selector")));
    assert!(!allowlist.contains(source.find(".after").expect("after selector")));
}

#[test]
fn multiple_closed_directives_allow_each_section_without_hiding_the_gap() {
    let source = format!(
        "{ALLOW_DUPLICATE_SELECTORS_START}\n.first {{}}\n\
         {ALLOW_DUPLICATE_SELECTORS_END}\n.between {{}}\n\
         {ALLOW_DUPLICATE_SELECTORS_START}\n.second {{}}\n\
         {ALLOW_DUPLICATE_SELECTORS_END}"
    );
    let allowlist = DuplicateSelectorAllowlist::from_source(&source);

    assert!(allowlist.contains(source.find(".first").expect("first allowed selector")));
    assert!(!allowlist.contains(source.find(".between").expect("selector between sections")));
    assert!(allowlist.contains(source.find(".second").expect("second allowed selector")));
}

#[test]
fn unclosed_duplicate_selector_directive_does_not_hide_later_rules() {
    let source = format!(".outside {{}}\n{ALLOW_DUPLICATE_SELECTORS_START}\n.still-checked {{}}");
    let allowlist = DuplicateSelectorAllowlist::from_source(&source);

    assert!(!allowlist.contains(
        source
            .find(".still-checked")
            .expect("selector after incomplete directive")
    ));
}
