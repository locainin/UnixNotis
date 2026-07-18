//! SVG parsing with all secondary resource resolvers disabled

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use super::decode::{decode_error, validate_dimensions};
use super::{AssetPolicy, IconAssetError, ResolvedIconAsset};

pub(super) fn decode_svg_icon(
    path: &Path,
    bytes: &[u8],
    policy: AssetPolicy,
    render_size: Option<u32>,
) -> Result<ResolvedIconAsset, IconAssetError> {
    // The default usvg resolver reads local image paths, so replace it before parsing untrusted SVG
    let embedded_image = Arc::new(AtomicBool::new(false));
    let data_image = Arc::clone(&embedded_image);
    let path_image = Arc::clone(&embedded_image);
    let options = resvg::usvg::Options {
        image_href_resolver: resvg::usvg::ImageHrefResolver {
            resolve_data: Box::new(move |_mime, _data, _options| {
                data_image.store(true, Ordering::Relaxed);
                None
            }),
            resolve_string: Box::new(move |_href, _options| {
                path_image.store(true, Ordering::Relaxed);
                None
            }),
        },
        ..resvg::usvg::Options::default()
    };
    let tree =
        resvg::usvg::Tree::from_data(bytes, &options).map_err(|error| decode_error(path, error))?;
    if embedded_image.load(Ordering::Relaxed) {
        // Reject instead of silently dropping the node so preset review stays honest
        return Err(IconAssetError::EmbeddedSvgImage(path.to_path_buf()));
    }
    let source_width = tree.size().width().ceil() as u32;
    let source_height = tree.size().height().ceil() as u32;
    // Source geometry is bounded before allocating the render surface
    validate_dimensions(path, source_width, source_height, policy)?;

    let max_render = render_size.unwrap_or(source_width.max(source_height));
    if max_render == 0 {
        return Err(IconAssetError::InvalidRenderSize);
    }
    let scale =
        (max_render as f32 / tree.size().width()).min(max_render as f32 / tree.size().height());
    // Rounded dimensions preserve the document ratio while keeping both axes nonzero
    let width = (tree.size().width() * scale).round().max(1.0) as u32;
    let height = (tree.size().height() * scale).round().max(1.0) as u32;
    let mut pixmap =
        resvg::tiny_skia::Pixmap::new(width, height).ok_or_else(|| IconAssetError::Decode {
            path: path.to_path_buf(),
            message: "could not allocate bounded SVG surface".to_string(),
        })?;
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );
    // tiny-skia exposes premultiplied RGBA which the GTK bridge handles explicitly
    Ok(ResolvedIconAsset {
        rgba: pixmap.take(),
        width,
        height,
        premultiplied_alpha: true,
    })
}

#[cfg(test)]
#[path = "tests/svg.rs"]
mod tests;
