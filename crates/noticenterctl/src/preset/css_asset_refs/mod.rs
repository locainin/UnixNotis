//! CSS asset discovery, validation, and bundle-only rewriting

mod collect;
mod model;
mod parse;
mod paths;
mod rewrite;
#[cfg(test)]
mod tests;

pub use collect::collect_external_css_asset_refs_from_paths;
pub(super) use collect::{
    collect_external_css_asset_refs_from_bundle, collect_external_css_asset_refs_from_collected,
    collect_local_css_asset_paths_from_captures,
};
pub use model::{ExternalCssAssetRef, HostSpecificCssAssetRef};
pub(super) use paths::{asset_path_reason, has_css_extension, local_file_url_path, read_css_text};
pub(super) use rewrite::rewrite_host_specific_css_asset_refs_in_sources;
