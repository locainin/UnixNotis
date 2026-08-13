//! Statistic grid construction tests

use super::flowbox_columns;

#[test]
fn grid_columns_normalize_zero_and_preserve_positive_values() {
    assert_eq!(flowbox_columns(0), 1);
    assert_eq!(flowbox_columns(1), 1);
    assert_eq!(flowbox_columns(4), 4);
}

#[test]
fn grid_columns_saturate_when_usize_exceeds_u32() {
    assert_eq!(flowbox_columns(usize::MAX), u32::MAX);
}
