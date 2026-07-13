#![allow(
    clippy::float_cmp,
    reason = "the parser returns exact decimal literals for these integer and finite CSS inputs"
)]

use std::collections::HashMap;

use super::{parse_box_edges, parse_box_vertical_edges, parse_single_length, set_edge};

#[test]
fn parse_single_length_resolves_calc_compare_and_var_fallbacks() {
    let mut properties = HashMap::new();
    properties.insert("--base".to_string(), "calc(10px + 2px)".to_string());
    properties.insert("--chosen".to_string(), "var(--missing, 18px)".to_string());

    assert_eq!(parse_single_length("var(--base)", &properties), Some(12.0));
    assert_eq!(
        parse_single_length("var(--chosen)", &properties),
        Some(18.0)
    );
    assert_eq!(
        parse_single_length("clamp(4px, max(12px, 14px), 20px)", &properties),
        Some(14.0)
    );
}

#[test]
fn parse_single_length_rejects_percentages_units_and_bad_math() {
    let properties = HashMap::new();

    assert_eq!(parse_single_length("80%", &properties), None);
    assert_eq!(parse_single_length("1rem", &properties), None);
    assert_eq!(parse_single_length("calc(10px / 0)", &properties), None);
    assert_eq!(parse_single_length("calc(10px + 2)", &properties), None);
}

#[test]
fn parse_box_edges_follows_css_horizontal_shorthand_rules() {
    let properties = HashMap::new();

    let one = parse_box_edges("3px", &properties).expect("one value");
    assert_eq!(one.left, 3.0);
    assert_eq!(one.right, 3.0);

    let two = parse_box_edges("1px 4px", &properties).expect("two values");
    assert_eq!(two.left, 4.0);
    assert_eq!(two.right, 4.0);

    let three = parse_box_edges("1px 4px 7px", &properties).expect("three values");
    assert_eq!(three.left, 4.0);
    assert_eq!(three.right, 4.0);

    let four = parse_box_edges("1px 2px 3px 4px", &properties).expect("four values");
    assert_eq!(four.left, 4.0);
    assert_eq!(four.right, 2.0);
}

#[test]
fn parse_box_vertical_edges_follows_css_vertical_shorthand_rules() {
    let properties = HashMap::new();

    let two = parse_box_vertical_edges("6px 9px", &properties).expect("two values");
    assert_eq!(two.top, 6.0);
    assert_eq!(two.bottom, 6.0);

    let three = parse_box_vertical_edges("1px 2px 3px", &properties).expect("three values");
    assert_eq!(three.top, 1.0);
    assert_eq!(three.bottom, 3.0);

    let four = parse_box_vertical_edges("1px 2px 3px 4px", &properties).expect("four values");
    assert_eq!(four.top, 1.0);
    assert_eq!(four.bottom, 3.0);
}

#[test]
fn set_edge_leaves_existing_value_when_length_cannot_resolve() {
    let properties = HashMap::new();
    let mut edge = 8.0;

    set_edge(&mut edge, "var(--missing)", &properties);
    assert_eq!(edge, 8.0);

    set_edge(&mut edge, "12px", &properties);
    assert_eq!(edge, 12.0);
}
