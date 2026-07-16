//! Export outcomes and injectable confirmation hooks

use std::path::PathBuf;

use anyhow::Result;

use super::checks::HostSpecificScriptLeak;
use crate::preset::command_rules::HostSpecificCommandPath;
use crate::preset::css_asset_refs::{ExternalCssAssetRef, HostSpecificCssAssetRef};

#[derive(Debug)]
pub(in crate::preset) struct ExportSummary {
    // Final bundle file path shown back to the CLI caller
    pub(super) bundle_path: PathBuf,
    // Count of regular files actually stored in the bundle
    pub(super) file_count: usize,
    // Symlinks are reported so the caller can clean them up if needed
    pub(super) skipped_symlinks: Vec<PathBuf>,
    // Non-regular paths are ignored because they are not portable preset content
    pub(super) skipped_non_regular: Vec<PathBuf>,
}

type ConfirmExternalCssRefsFn = fn(&[ExternalCssAssetRef]) -> Result<()>;
type PromptFixCommandPathsFn = fn(&[HostSpecificCommandPath]) -> Result<bool>;
type PromptFixCssAssetRefsFn = fn(&[HostSpecificCssAssetRef]) -> Result<bool>;
type PromptFixScriptPathsFn = fn(&[HostSpecificScriptLeak]) -> Result<bool>;

pub(in crate::preset) struct ExportConfirmers {
    // Guard for CSS refs that leave the config root
    pub(super) confirm_external_css_refs: ConfirmExternalCssRefsFn,
    // Prompt hook for command path rewrite flow
    pub(super) prompt_fix_host_specific_command_paths: PromptFixCommandPathsFn,
    // Prompt hook for CSS asset rewrite flow
    pub(super) prompt_fix_host_specific_css_asset_refs: PromptFixCssAssetRefsFn,
    // Prompt hook for script text rewrite flow
    pub(super) prompt_fix_host_specific_script_paths: PromptFixScriptPathsFn,
}

#[cfg(test)]
#[path = "tests/model.rs"]
mod tests;
