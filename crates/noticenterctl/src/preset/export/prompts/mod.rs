//! Export warnings, confirmations, and optional portability rewrites

mod interactive;
mod rewrite;

pub(in crate::preset::export) use interactive::{
    confirm_export_external_css_refs, prompt_to_fix_host_specific_command_paths,
    prompt_to_fix_host_specific_css_asset_refs, prompt_to_fix_host_specific_script_paths,
};
pub(in crate::preset::export) use rewrite::{
    rewrite_host_specific_command_paths_if_requested,
    rewrite_host_specific_css_asset_refs_if_requested,
    rewrite_host_specific_script_paths_if_requested,
};
