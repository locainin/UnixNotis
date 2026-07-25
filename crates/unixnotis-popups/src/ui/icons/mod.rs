//! Popup icon lookup, decoding, and cache ownership

mod cache;
mod decode;
mod resolver;

pub(super) use cache::{IconDecodePool, IconDecodeResult, TextureCache};
pub(super) use decode::{decode_icon_file, RasterIcon};
pub(super) use resolver::{collect_icon_candidates, file_path_from_hint, resolve_icon_image};
