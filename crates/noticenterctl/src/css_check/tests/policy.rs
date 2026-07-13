use super::super::policy::{
    is_horizontal_size_property, is_vertical_size_property, parsing_error_hint,
};

#[test]
fn size_property_classifiers_keep_width_and_height_sets_distinct() {
    assert!(is_horizontal_size_property("width"));
    assert!(is_horizontal_size_property("border-right-width"));
    assert!(!is_horizontal_size_property("height"));

    assert!(is_vertical_size_property("height"));
    assert!(is_vertical_size_property("border-bottom-width"));
    assert!(!is_vertical_size_property("min-width"));
}

#[test]
fn parsing_error_hint_reports_targeted_layout_value_guidance() {
    assert!(parsing_error_hint("width: 80%;")
        .expect("percentage hint")
        .contains("percentage sizing"));
    assert!(parsing_error_hint("width: calc(10px + 2px);")
        .expect("calc hint")
        .contains("GTK supports calc()"));
    assert!(parsing_error_hint("padding: var(--gap);")
        .expect("var hint")
        .contains("custom properties need"));
    assert!(parsing_error_hint("color: red;").is_none());
}
