use super::super::size_rules::{
    normalized_horizontal_size_rules, should_warn_for_unmodeled_known_class,
};

fn props(values: &[(&str, &str)]) -> Vec<(String, String)> {
    values
        .iter()
        .map(|(name, value)| ((*name).to_string(), (*value).to_string()))
        .collect()
}

#[test]
fn normalized_horizontal_size_rules_keeps_later_duplicate_value() {
    let rules = normalized_horizontal_size_rules(&props(&[
        ("width", "10px"),
        ("color", "red"),
        ("width", "12px"),
        ("padding-left", " 2px "),
    ]));

    assert_eq!(rules.get("width").map(String::as_str), Some("12px"));
    assert_eq!(rules.get("padding-left").map(String::as_str), Some("2px"));
    assert!(!rules.contains_key("color"));
}

#[test]
fn small_known_badge_width_stays_quiet_but_large_width_warns() {
    assert!(!should_warn_for_unmodeled_known_class(
        ".unixnotis-panel-count",
        &props(&[("width", "24px"), ("padding", "2px 4px")])
    ));
    assert!(should_warn_for_unmodeled_known_class(
        ".unixnotis-panel-count",
        &props(&[("width", "96px")])
    ));
}

#[test]
fn non_size_rules_do_not_warn_for_unmodeled_known_class() {
    assert!(!should_warn_for_unmodeled_known_class(
        ".unixnotis-panel-action",
        &props(&[("color", "red")])
    ));
}
