//! Caller-supplied notification content image decoding

use gtk::gdk;
use gtk::glib::object::Cast;
use unixnotis_core::{ImageData, NotificationImage};

pub(in crate::ui) fn image_data_texture(image: &NotificationImage) -> Option<gdk::Texture> {
    // Content images stay separate from the authenticated application badge
    if !image.has_image_data {
        return None;
    }

    let data = &image.image_data;
    // GTK memory textures need positive dimensions and eight-bit channels
    if data.bits_per_sample != 8 || data.rowstride < 0 || data.width <= 0 || data.height <= 0 {
        return None;
    }

    let width = usize::try_from(data.width).ok()?;
    let height = usize::try_from(data.height).ok()?;
    let width_i32 = i32::try_from(width).ok()?;
    let height_i32 = i32::try_from(height).ok()?;

    let (bytes, stride) = match data.channels {
        4 => {
            // Row padding is valid, but every visible pixel must fit in each row
            let min_stride = width.checked_mul(4)?;
            let stride = if data.rowstride > 0 {
                usize::try_from(data.rowstride).ok()?
            } else {
                min_stride
            };
            if stride < min_stride || data.data.len() < stride.checked_mul(height)? {
                return None;
            }
            (gtk::glib::Bytes::from(&data.data), stride)
        }
        3 => {
            // GTK has no matching packed RGB format here, so add an opaque alpha channel
            let (expanded, stride) = expand_rgb_to_rgba(data)?;
            (gtk::glib::Bytes::from(&expanded), stride)
        }
        _ => return None,
    };

    Some(
        gdk::MemoryTexture::new(
            width_i32,
            height_i32,
            gdk::MemoryFormat::R8g8b8a8,
            &bytes,
            stride,
        )
        .upcast::<gdk::Texture>(),
    )
}

fn expand_rgb_to_rgba(data: &ImageData) -> Option<(Vec<u8>, usize)> {
    // Every multiplication is checked before allocating or slicing image storage
    let width = usize::try_from(data.width).ok()?;
    let height = usize::try_from(data.height).ok()?;
    if width == 0 || height == 0 {
        return None;
    }

    let min_source_stride = width.checked_mul(3)?;
    let source_stride = if data.rowstride > 0 {
        usize::try_from(data.rowstride).ok()?
    } else {
        min_source_stride
    };
    if source_stride < min_source_stride || data.data.len() < source_stride.checked_mul(height)? {
        return None;
    }

    let target_stride = width.checked_mul(4)?;
    let mut rgba = vec![0; target_stride.checked_mul(height)?];
    for row in 0..height {
        // Source padding is skipped while target rows remain tightly packed
        let source_start = row.checked_mul(source_stride)?;
        let target_start = row.checked_mul(target_stride)?;
        let source = &data.data[source_start..source_start + min_source_stride];
        let target = &mut rgba[target_start..target_start + target_stride];
        for column in 0..width {
            let source_pixel = column * 3;
            let target_pixel = column * 4;
            target[target_pixel..target_pixel + 3]
                .copy_from_slice(&source[source_pixel..source_pixel + 3]);
            target[target_pixel + 3] = u8::MAX;
        }
    }

    Some((rgba, target_stride))
}

#[cfg(test)]
#[path = "tests/content.rs"]
mod tests;
