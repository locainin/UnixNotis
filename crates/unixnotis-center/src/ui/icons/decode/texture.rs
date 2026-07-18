//! GTK texture construction from worker-owned pixel buffers

use gtk::gdk::{self, Texture};
use gtk::glib;
use gtk::prelude::*;

use super::model::RasterImage;

pub(in crate::ui::icons) fn texture_from_raster(image: &RasterImage) -> Texture {
    let bytes = glib::Bytes::from(&image.bytes);
    let format = if image.premultiplied_alpha {
        gdk::MemoryFormat::R8g8b8a8Premultiplied
    } else {
        gdk::MemoryFormat::R8g8b8a8
    };
    gdk::MemoryTexture::new(
        image.width,
        image.height,
        format,
        &bytes,
        usize::try_from(image.stride).unwrap_or_default(),
    )
    .upcast::<Texture>()
}
