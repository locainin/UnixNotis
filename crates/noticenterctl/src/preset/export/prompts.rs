//! Export warning and fix prompts
//!
//! Export uses these helpers to keep user-facing warnings out of the orchestration flow

use anyhow::{anyhow, Result};
use std::io::IsTerminal;
use std::path::Path;
use unixnotis_core::Config;

use crate::preset::command_rules::{
    collect_host_specific_command_paths, rewrite_host_specific_command_paths,
    HostSpecificCommandPath,
};
use crate::preset::css_asset_refs::ExternalCssAssetRef;
use crate::preset::pathing::{confirm_continue_or_abort, prompt_yes_no};

pub(super) fn confirm_export_external_css_refs(
    external_refs: &[ExternalCssAssetRef],
) -> Result<()> {
    if external_refs.is_empty() {
        return Ok(());
    }

    // The warning prints first so the caller can see exactly which CSS file caused the prompt
    let details = format_external_css_ref_lines(external_refs);
    eprintln!(
        "preset export warning: found {} CSS asset reference(s) outside the UnixNotis config directory",
        external_refs.len()
    );
    for line in &details {
        eprintln!("{line}");
    }

    confirm_continue_or_abort(
        "External CSS asset references were found; continue exporting anyway?",
        &format!(
            "preset export found CSS asset references outside the UnixNotis config directory; rerun interactively to confirm anyway\n{}",
            details.join("\n")
        ),
    )
}

pub(super) fn rewrite_host_specific_command_paths_if_requested<G>(
    config_dir: &Path,
    config: &mut Config,
    prompt_fix_host_specific_command_paths: G,
) -> Result<Vec<HostSpecificCommandPath>>
where
    G: FnOnce(&[HostSpecificCommandPath]) -> Result<bool>,
{
    let leaked_paths = collect_host_specific_command_paths(config_dir, config);
    if leaked_paths.is_empty() {
        return Ok(Vec::new());
    }

    // The warning always shows first so the caller can decide with the real slot names in view
    let details = format_host_specific_command_path_lines(&leaked_paths);
    eprintln!(
        "preset export warning: found {} host-specific command path(s) under the UnixNotis config directory",
        leaked_paths.len()
    );
    for line in &details {
        eprintln!("{line}");
    }

    // Declining the helper keeps the bundle valid, but the warning stays visible
    if !prompt_fix_host_specific_command_paths(&leaked_paths)? {
        eprintln!(
            "preset export warning: leaving host-specific command paths unchanged in the bundle"
        );
        return Ok(Vec::new());
    }

    Ok(rewrite_host_specific_command_paths(config_dir, config))
}

pub(super) fn prompt_to_fix_host_specific_command_paths(
    _leaked_paths: &[HostSpecificCommandPath],
) -> Result<bool> {
    if std::io::stdin().is_terminal() && std::io::stdout().is_terminal() {
        // Interactive export can offer the fix directly instead of forcing a second command
        return prompt_yes_no(
            "Host-specific command paths were found; let noticenterctl rewrite them in the exported preset?",
        );
    }

    // Non-interactive export should not silently decide whether to rewrite command paths
    Err(anyhow!(
        "preset export found host-specific command paths under the UnixNotis config directory; rerun interactively to let noticenterctl rewrite them"
    ))
}

fn format_external_css_ref_lines(external_refs: &[ExternalCssAssetRef]) -> Vec<String> {
    external_refs
        .iter()
        .map(|asset_ref| {
            // One line per asset keeps warning output readable when several files are involved
            format!(
                "  - {} -> {} ({})",
                asset_ref.css_file.display(),
                asset_ref.asset_ref,
                asset_ref.reason
            )
        })
        .collect()
}

fn format_host_specific_command_path_lines(
    leaked_paths: &[HostSpecificCommandPath],
) -> Vec<String> {
    leaked_paths
        .iter()
        .map(|leak| {
            // These rows point straight at the leaking slot so the config can be rewritten cleanly
            format!(
                "  - {} = {} (absolute path under the config root; let noticenterctl rewrite it to a config-root-relative command)",
                leak.slot, leak.command
            )
        })
        .collect()
}
