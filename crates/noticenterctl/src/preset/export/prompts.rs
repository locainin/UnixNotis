//! Export warning and fix prompts
//!
//! Export uses these helpers to keep user-facing warnings out of the main flow

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
use crate::preset::export::checks::{
    clear_script_overrides, rewrite_host_specific_script_paths_in_sources, HostSpecificScriptLeak,
};
use crate::preset::pathing::{confirm_continue_or_abort, prompt_yes_no};

pub(super) fn confirm_export_external_css_refs(
    external_refs: &[ExternalCssAssetRef],
) -> Result<()> {
    if external_refs.is_empty() {
        return Ok(());
    }

    // Print full context before asking to continue
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

    // Show exact command slots so the choice is explicit
    let details = format_host_specific_command_path_lines(&leaked_paths);
    eprintln!(
        "preset export warning: found {} host-specific command path(s) under the UnixNotis config directory",
        leaked_paths.len()
    );
    for line in &details {
        eprintln!("{line}");
    }

    // Decline keeps current values and still exports
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
    // CSS rewrite happens in-memory only
    let leaked_refs =
        rewrite_host_specific_css_asset_refs_in_sources(config_dir, &mut collected.files)?;
    if leaked_refs.is_empty() {
        return Ok(Vec::new());
    }

    // Show exact url(...) values before keeping or dropping rewrites
    let details = format_host_specific_css_asset_ref_lines(&leaked_refs);
    eprintln!(
        "preset export warning: found {} host-specific CSS asset reference(s) under the UnixNotis config directory",
        leaked_refs.len()
    );
    for line in &details {
        eprintln!("{line}");
    }

    if prompt_fix_host_specific_css_asset_refs(&leaked_refs)? {
        // Keep staged rewrite bytes
        return Ok(leaked_refs);
    }

    // Drop staged rewrite bytes and keep original CSS content
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

pub(super) fn rewrite_host_specific_script_paths_if_requested<G>(
    config_dir: &Path,
    collected: &mut CollectedConfigFiles,
    prompt_fix_host_specific_script_paths: G,
) -> Result<Vec<HostSpecificScriptLeak>>
where
    G: FnOnce(&[HostSpecificScriptLeak]) -> Result<bool>,
{
    // Script rewrite also stays in-memory so live scripts are not touched
    let leaked_refs =
        rewrite_host_specific_script_paths_in_sources(config_dir, &mut collected.files)?;
    if leaked_refs.is_empty() {
        return Ok(Vec::new());
    }

    // Print one line per leak so prompt output is easy to scan
    let details = format_host_specific_script_path_lines(&leaked_refs);
    eprintln!(
        "preset export warning: found {} host-specific script path reference(s) under the UnixNotis config directory",
        leaked_refs.len()
    );
    for line in &details {
        eprintln!("{line}");
    }

    if prompt_fix_host_specific_script_paths(&leaked_refs)? {
        // Keep staged script rewrites
        return Ok(leaked_refs);
    }

    // Drop staged script rewrites and archive original bytes
    clear_script_overrides(&mut collected.files, &leaked_refs);
    eprintln!(
        "preset export warning: leaving host-specific script path references unchanged in the bundle"
    );
    Ok(Vec::new())
}

pub(super) fn prompt_to_fix_host_specific_command_paths(
    _leaked_paths: &[HostSpecificCommandPath],
) -> Result<bool> {
    if std::io::stdin().is_terminal() && std::io::stdout().is_terminal() {
        // Interactive mode can ask and continue in one run
        return prompt_yes_no(
            "Host-specific command paths were found; let noticenterctl rewrite them in the exported preset?",
        );
    }

    // Non-interactive mode cannot guess rewrite intent
    Err(anyhow!(
        "preset export found host-specific command paths under the UnixNotis config directory; rerun interactively to let noticenterctl rewrite them"
    ))
}

pub(super) fn prompt_to_fix_host_specific_css_asset_refs(
    _leaked_refs: &[HostSpecificCssAssetRef],
) -> Result<bool> {
    if std::io::stdin().is_terminal() && std::io::stdout().is_terminal() {
        // Interactive mode can ask and continue in one run
        return prompt_yes_no(
            "Host-specific CSS asset references were found; let noticenterctl rewrite them in the exported preset?",
        );
    }

    Err(anyhow!(
        "preset export found host-specific CSS asset references under the UnixNotis config directory; rerun interactively to let noticenterctl rewrite them"
    ))
}

pub(super) fn prompt_to_fix_host_specific_script_paths(
    _leaked_refs: &[HostSpecificScriptLeak],
) -> Result<bool> {
    if std::io::stdin().is_terminal() && std::io::stdout().is_terminal() {
        // Interactive mode can ask and continue in one run
        return prompt_yes_no(
            "Host-specific script paths were found; let noticenterctl rewrite them in the exported preset?",
        );
    }

    // Non-interactive mode cannot guess rewrite intent
    Err(anyhow!(
        "preset export found host-specific script path references under the UnixNotis config directory; rerun interactively to let noticenterctl rewrite them"
    ))
}

fn format_external_css_ref_lines(external_refs: &[ExternalCssAssetRef]) -> Vec<String> {
    external_refs
        .iter()
        .map(|asset_ref| {
            // One warning line per found reference
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
            // Show exact slot and command for quick review
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
            // Include target rewrite so prompt answer is clear
            format!(
                "  - {} -> {} (host-local config path; let noticenterctl rewrite it to {})",
                leak.css_file.display(),
                leak.asset_ref,
                leak.rewritten_ref
            )
        })
        .collect()
}

fn format_host_specific_script_path_lines(leaked_refs: &[HostSpecificScriptLeak]) -> Vec<String> {
    leaked_refs
        .iter()
        .map(|leak| {
            // Include replacement text to show final form
            format!(
                "  - {} contains {} (let noticenterctl rewrite it to {})",
                leak.script_path.display(),
                leak.needle,
                leak.rewritten_to
            )
        })
        .collect()
}
