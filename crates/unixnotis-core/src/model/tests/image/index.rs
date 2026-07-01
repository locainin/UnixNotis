use zbus::zvariant::{OwnedValue, Structure, Value};

mod hints;
mod normalization;
mod projection;
mod rgb_expansion;

fn string_value(value: &str) -> OwnedValue {
    Value::from(value)
        .try_into()
        .expect("string value should convert")
}

fn image_data_value(
    width: i32,
    height: i32,
    rowstride: i32,
    has_alpha: bool,
    bits_per_sample: i32,
    channels: i32,
    data: Vec<u8>,
) -> OwnedValue {
    // Notification image-data is the standard (iiibiiay) D-Bus structure
    let structure = Structure::from((
        width,
        height,
        rowstride,
        has_alpha,
        bits_per_sample,
        channels,
        data,
    ));
    Value::from(structure)
        .try_into()
        .expect("image structure should convert")
}
