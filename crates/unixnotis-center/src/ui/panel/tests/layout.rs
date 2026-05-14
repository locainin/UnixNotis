use super::height_from_percent;

#[test]
fn height_from_percent_scales_usable_height() {
    assert_eq!(height_from_percent(1000, 84), 840);
    assert_eq!(height_from_percent(701, 84), 589);
}

#[test]
fn height_from_percent_keeps_a_positive_floor() {
    assert_eq!(height_from_percent(1, 1), 1);
    assert_eq!(height_from_percent(40, 1), 1);
}
