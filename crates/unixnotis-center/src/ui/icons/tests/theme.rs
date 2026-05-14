use super::expand_rgb_to_rgba;
use unixnotis_core::ImageData;

#[test]
fn expand_rgb_to_rgba_appends_alpha() {
    let data = ImageData {
        width: 2,
        height: 1,
        rowstride: 0,
        has_alpha: false,
        bits_per_sample: 8,
        channels: 3,
        data: vec![10, 20, 30, 40, 50, 60],
    };
    let (expanded, stride) = expand_rgb_to_rgba(&data).expect("rgb expansion");
    assert_eq!(stride, 8);
    assert_eq!(expanded, vec![10, 20, 30, 255, 40, 50, 60, 255]);
}

#[test]
fn expand_rgb_to_rgba_honors_row_padding() {
    let data = ImageData {
        width: 2,
        height: 1,
        rowstride: 8,
        has_alpha: false,
        bits_per_sample: 8,
        channels: 3,
        data: vec![1, 2, 3, 4, 5, 6, 99, 100],
    };
    let (expanded, stride) = expand_rgb_to_rgba(&data).expect("rgb expansion");
    assert_eq!(stride, 8);
    assert_eq!(expanded, vec![1, 2, 3, 255, 4, 5, 6, 255]);
}
