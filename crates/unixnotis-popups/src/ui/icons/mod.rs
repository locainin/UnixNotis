//! Popup icon lookup, decoding, and cache ownership

mod cache;
mod content;
mod decode;
mod resolver;
mod state;
mod theme_cache;

pub(super) use cache::{IconDecodePool, IconDecodeResult, TextureCache};
pub(super) use content::{image_data_texture, image_data_texture_for_data};
pub(super) use decode::{decode_icon_file, RasterIcon};
pub(super) use resolver::{collect_icon_candidates, file_path_from_hint};
pub(super) use theme_cache::ThemeIconCache;
