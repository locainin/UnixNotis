use super::*;

#[test]
fn large_wire_avatar_is_downsampled_before_model_validation() {
    let wire = WireImageData::from_parts(320, 320, 320 * 4, true, 8, 4, vec![17_u8; 320 * 320 * 4])
        .expect("320x320 wire avatar should be valid");

    let image = wire
        .into_storage_image(64)
        .expect("valid wire avatar should be reduced to storage size");

    assert_eq!((image.width, image.height), (64, 64));
    assert_eq!(image.data.len(), 64 * 64 * 4);
    assert!(image.data.iter().all(|byte| *byte == 17));
}

#[test]
fn non_square_wire_images_preserve_aspect_ratio_during_downsampling() {
    let wire = WireImageData::from_parts(320, 160, 320 * 4, true, 8, 4, vec![23_u8; 320 * 160 * 4])
        .expect("non-square wire image should be valid");

    let image = wire
        .into_storage_image(64)
        .expect("non-square wire image should normalize");

    assert_eq!((image.width, image.height), (64, 32));
    assert_eq!(image.data.len(), 64 * 32 * 4);
}

#[test]
fn maximum_wire_raster_is_reduced_to_the_content_model_bound() {
    let wire = WireImageData::from_parts(
        1024,
        1024,
        1024 * 4,
        true,
        8,
        4,
        vec![31_u8; MAX_NOTIFY_WIRE_IMAGE_BYTES],
    )
    .expect("maximum documented wire raster should be valid");

    let image = wire
        .into_storage_image(256)
        .expect("maximum wire raster should be reduced before storage");

    assert_eq!((image.width, image.height), (256, 256));
    assert_eq!(image.data.len(), 256 * 256 * 4);
}

#[test]
fn padded_rgb_wire_rows_are_tightly_packed_as_rgba() {
    let mut data = vec![0xee_u8; 2 * 8];
    data[..6].copy_from_slice(&[1, 2, 3, 4, 5, 6]);
    data[8..14].copy_from_slice(&[7, 8, 9, 10, 11, 12]);
    let wire = WireImageData::from_parts(2, 2, 8, false, 8, 3, data)
        .expect("padded RGB rows should be valid");

    let image = wire
        .into_storage_image(2)
        .expect("padded RGB rows should normalize");

    assert_eq!(
        image.data,
        [1, 2, 3, 255, 4, 5, 6, 255, 7, 8, 9, 255, 10, 11, 12, 255]
    );
}

#[test]
fn alpha_flag_and_channel_count_must_describe_the_same_layout() {
    assert!(WireImageData::from_parts(1, 1, 4, false, 8, 4, vec![0; 4]).is_none());
    assert!(WireImageData::from_parts(1, 1, 3, true, 8, 3, vec![0; 3]).is_none());
}

#[test]
fn final_wire_row_does_not_require_trailing_stride_padding() {
    assert!(WireImageData::from_parts(1, 2, 4, false, 8, 3, vec![0; 7]).is_some());
    assert!(WireImageData::from_parts(1, 2, 4, false, 8, 3, vec![0; 6]).is_none());
}

#[test]
fn wire_image_metadata_and_bounds_fail_closed() {
    let valid_data = vec![0_u8; 4];
    assert!(WireImageData::from_parts(0, 1, 4, true, 8, 4, valid_data.clone()).is_none());
    assert!(WireImageData::from_parts(1, 0, 4, true, 8, 4, valid_data.clone()).is_none());
    assert!(WireImageData::from_parts(1025, 1, 4100, true, 8, 4, vec![0; 4100]).is_none());
    assert!(WireImageData::from_parts(1, 1, 4, true, 16, 4, valid_data.clone()).is_none());
    assert!(WireImageData::from_parts(1, 1, 4, true, 8, 2, valid_data.clone()).is_none());
    assert!(WireImageData::from_parts(1, 1, 3, true, 8, 4, valid_data.clone()).is_none());
    assert!(WireImageData::from_parts(1, 1, 4, true, 8, 4, vec![0; 3]).is_none());
    assert!(WireImageData::from_parts(
        1,
        1,
        4,
        true,
        8,
        4,
        vec![0; MAX_NOTIFY_WIRE_IMAGE_BYTES + 1]
    )
    .is_none());
    assert!(WireImageData::from_parts(1, 1, 4, true, 8, 4, valid_data)
        .expect("valid image")
        .into_storage_image(0)
        .is_none());
}

#[test]
fn byte_array_decoder_reports_the_expected_input_shape() {
    let error = BoundedImageBytes::deserialize(serde::de::value::UnitDeserializer::<
        serde::de::value::Error,
    >::new())
    .expect_err("unit input is not a byte sequence");

    assert!(error
        .to_string()
        .contains("a bounded notification image byte array"));
}
