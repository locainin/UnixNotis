use super::super::limits::{
    MAX_IMAGE_BYTES, MAX_NESTED_CONTAINER_ELEMENTS, MAX_NON_IMAGE_ARRAY_BYTES,
    MAX_NON_IMAGE_STRING_BYTES, MAX_SIGNATURE_DEPTH,
};

#[test]
fn raw_body_limits_keep_the_reviewed_byte_and_depth_boundaries() {
    assert_eq!(MAX_IMAGE_BYTES, 262_144);
    assert_eq!(MAX_NON_IMAGE_ARRAY_BYTES, 16_384);
    assert_eq!(MAX_NON_IMAGE_STRING_BYTES, 65_536);
    assert_eq!(MAX_NESTED_CONTAINER_ELEMENTS, 64);
    assert_eq!(MAX_SIGNATURE_DEPTH, 16);
}
