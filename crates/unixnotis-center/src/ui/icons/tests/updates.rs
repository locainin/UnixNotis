use super::decoded_target;

#[test]
fn decoded_target_multiplies_logical_size_by_scale() {
    assert_eq!(decoded_target(24, 2), 48);
}

#[test]
fn decoded_target_clamps_invalid_geometry_to_one_pixel() {
    assert_eq!(decoded_target(0, 0), 1);
    assert_eq!(decoded_target(-8, -2), 1);
}
