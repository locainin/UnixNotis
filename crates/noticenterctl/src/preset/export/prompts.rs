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
use crate::preset::config_root::CollectedConfigFiles;
use crate::preset::css_asset_refs::{
    rewrite_host_specific_css_asset_refs_in_sources, ExternalCssAssetRef, HostSpecificCssAssetRef,
};
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

pub(super) fn rewrite_host_specific_css_asset_refs_if_requested<G>(
    config_dir: &Path,
    collected: &mut CollectedConfigFiles,
    prompt_fix_host_specific_css_asset_refs: G,
) -> Result<Vec<HostSpecificCssAssetRef>>
where
    G: FnOnce(&[HostSpecificCssAssetRef]) -> Result<bool>,
{
    let leaked_refs =
        rewrite_host_specific_css_asset_refs_in_sources(config_dir, &mut collected.files)?;
    if leaked_refs.is_empty() {
        return Ok(Vec::new());
    }

    // The warning shows the exact stylesheet and url(...) payload before any rewrite is kept
    let details = format_host_specific_css_asset_ref_lines(&leaked_refs);
    eprintln!(
        "preset export warning: found {} host-specific CSS asset reference(s) under the UnixNotis config directory",
        leaked_refs.len()
    );
    for line in &details {
        eprintln!("{line}");
    }

    if prompt_fix_host_specific_css_asset_refs(&leaked_refs)? {
        return Ok(leaked_refs);
    }

    // Declining the helper keeps the bundle valid, but the temporary overrides must be cleared
    for leaked_ref in &leaked_refs {
        if let Some(file) = collected
            .files
            .iter_mut()
            .find(|file| file.source_path == leaked_ref.css_file)
        {
            file.contents_override = None;
            if let Ok(metadata) = std::fs::metadata(&file.source_path) {
                file.size = metadata.len();
            }
        }
    }
    eprintln!(
        "preset export warning: leaving host-specific CSS asset references unchanged in the bundle"
    );
    Ok(Vec::new())
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

pub(super) fn prompt_to_fix_host_specific_css_asset_refs(
    _leaked_refs: &[HostSpecificCssAssetRef],
) -> Result<bool> {
    if std::io::stdin().is_terminal() && std::io::stdout().is_terminal() {
        // Interactive export can offer the CSS cleanup directly instead of leaking it into the bundle
        return prompt_yes_no(
            "Host-specific CSS asset references were found; let noticenterctl rewrite them in the exported preset?",
        );
    }

    Err(anyhow!(
        "preset export found host-specific CSS asset references under the UnixNotis config directory; rerun interactively to let noticenterctl rewrite them"
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

fn format_host_specific_css_asset_ref_lines(
    leaked_refs: &[HostSpecificCssAssetRef],
) -> Vec<String> {
    leaked_refs
        .iter()
        .map(|leak| {
            // One row per rewrite keeps the prompt readable even when one stylesheet leaks several assets
            format!(
                "  - {} -> {} (host-local config path; let noticenterctl rewrite it to {})",
                leak.css_file.display(),
                leak.asset_ref,
                leak.rewritten_ref
            )
        })
        .collect()
}
