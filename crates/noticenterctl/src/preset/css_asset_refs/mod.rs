//! CSS asset discovery, validation, and bundle-only rewriting

mod collect;
mod file_url;
mod model;
mod parse;
mod paths;
mod rewrite;

pub use collect::collect_external_css_asset_refs_from_paths;
pub(super) use collect::{
    collect_external_css_asset_refs_from_bundle, collect_external_css_asset_refs_from_collected,
    collect_local_css_asset_paths_from_captures,
};
pub(crate) use file_url::{classify_file_url, FileUrlClassification};
pub use model::{ExternalCssAssetRef, HostSpecificCssAssetRef};
pub use parse::{collect_import_dependency_values, CssImportReference};
pub(super) use paths::{asset_path_reason, has_css_extension, read_css_text};
pub use paths::{read_css_file_bounded, read_css_path_text_bounded};
pub(super) use rewrite::rewrite_host_specific_css_asset_refs_in_sources;
