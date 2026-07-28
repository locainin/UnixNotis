#![expect(
    clippy::float_cmp,
    reason = "the parser returns exact finite values for these integer CSS inputs"
)]

use std::collections::HashMap;

use super::super::{parse_box_edges, parse_box_vertical_edges, parse_single_length, set_edge};

#[test]
fn parse_single_length_uses_the_first_resolved_shorthand_token() {
    let properties = HashMap::new();

    assert_eq!(parse_single_length("invalid 12px", &properties), Some(12.0));
    assert_eq!(parse_single_length("80% 1rem", &properties), None);
}

#[test]
fn parse_box_edges_follows_every_css_horizontal_shorthand_shape() {
    let properties = HashMap::new();

    let one = parse_box_edges("3px", &properties).expect("one value");
    assert_eq!((one.left, one.right), (3.0, 3.0));

    let two = parse_box_edges("1px 4px", &properties).expect("two values");
    assert_eq!((two.left, two.right), (4.0, 4.0));

    let three = parse_box_edges("1px 4px 7px", &properties).expect("three values");
    assert_eq!((three.left, three.right), (4.0, 4.0));

    let four = parse_box_edges("1px 2px 3px 4px", &properties).expect("four values");
    assert_eq!((four.left, four.right), (4.0, 2.0));
}

#[test]
fn parse_box_vertical_edges_follows_every_css_vertical_shorthand_shape() {
    let properties = HashMap::new();

    let one = parse_box_vertical_edges("3px", &properties).expect("one value");
    assert_eq!((one.top, one.bottom), (3.0, 3.0));

    let two = parse_box_vertical_edges("6px 9px", &properties).expect("two values");
    assert_eq!((two.top, two.bottom), (6.0, 6.0));

    let three = parse_box_vertical_edges("1px 2px 3px", &properties).expect("three values");
    assert_eq!((three.top, three.bottom), (1.0, 3.0));

    let four = parse_box_vertical_edges("1px 2px 3px 4px", &properties).expect("four values");
    assert_eq!((four.top, four.bottom), (1.0, 3.0));
}

#[test]
fn malformed_or_oversized_shorthands_fail_closed() {
    let properties = HashMap::new();

    assert!(parse_box_edges("", &properties).is_none());
    assert!(parse_box_edges("1px 2px 3px 4px 5px", &properties).is_some());
    assert!(parse_box_vertical_edges("var(unterminated", &properties).is_none());
}

#[test]
fn set_edge_changes_only_resolved_lengths() {
    let properties = HashMap::new();
    let mut edge = 8.0;

    set_edge(&mut edge, "var(--missing)", &properties);
    assert_eq!(edge, 8.0);

    set_edge(&mut edge, "12px", &properties);
    assert_eq!(edge, 12.0);
}
