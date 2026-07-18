//! Widget icon asset validation and bounded decoding

mod decode;
mod error;
mod materialize;
mod model;
mod path;
mod raster;
mod resolver;
mod svg;

pub use decode::{decode_image_asset_contents, validate_icon_asset_contents};
pub use error::IconAssetError;
pub use materialize::materialize_bounded_image_as_png;
pub use model::{
    AssetPolicy, ResolvedIconAsset, DEFAULT_ICON_ASSET_EXTENSIONS, DEFAULT_ICON_ASSET_MAX_BYTES,
    DEFAULT_ICON_ASSET_MAX_HEIGHT, DEFAULT_ICON_ASSET_MAX_PIXELS, DEFAULT_ICON_ASSET_MAX_WIDTH,
};
pub use path::validate_icon_asset_reference;
pub use resolver::{
    resolve_icon_asset_path, resolve_icon_asset_path_with_policy, IconAssetResolver,
};
