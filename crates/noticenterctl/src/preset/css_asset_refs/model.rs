//! CSS asset findings returned by preset validation

use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalCssAssetRef {
    // CSS file that carried the outside asset reference
    pub(crate) css_file: PathBuf,
    // Raw url payload as written in the stylesheet
    pub(crate) asset_ref: String,
    // Short reason shown back to the caller
    pub(crate) reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostSpecificCssAssetRef {
    // CSS file that carried the host-local config path
    pub(crate) css_file: PathBuf,
    // Raw url payload as written in the stylesheet
    pub(crate) asset_ref: String,
    // Replacement path written into the bundled stylesheet
    pub(crate) rewritten_ref: String,
}

#[cfg(test)]
#[path = "tests/model.rs"]
mod tests;
