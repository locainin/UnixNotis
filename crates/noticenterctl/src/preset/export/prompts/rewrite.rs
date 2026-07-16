//! Optional in-memory portability rewrites

use anyhow::Result;
use std::path::Path;
use unixnotis_core::Config;

use crate::preset::command_rules::{
    collect_host_specific_command_paths, rewrite_host_specific_command_paths,
    HostSpecificCommandPath,
};
use crate::preset::config_root::CollectedConfigFiles;
use crate::preset::css_asset_refs::{
    rewrite_host_specific_css_asset_refs_in_sources, HostSpecificCssAssetRef,
};
use crate::preset::export::checks::{
    capture_file_overrides, restore_file_overrides, rewrite_host_specific_script_paths_in_sources,
    HostSpecificScriptLeak,
};
pub(in crate::preset::export) fn rewrite_host_specific_command_paths_if_requested<G>(
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

pub(in crate::preset::export) fn rewrite_host_specific_css_asset_refs_if_requested<G>(
    config_dir: &Path,
    collected: &mut CollectedConfigFiles,
    prompt_fix_host_specific_css_asset_refs: G,
) -> Result<Vec<HostSpecificCssAssetRef>>
where
    G: FnOnce(&[HostSpecificCssAssetRef]) -> Result<bool>,
{
    let snapshots = capture_file_overrides(&collected.files);

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

    match prompt_fix_host_specific_css_asset_refs(&leaked_refs) {
        Ok(true) => {
            // Keep staged rewrite bytes
            Ok(leaked_refs)
        }
        Ok(false) => {
            // Declining rewrite restores the exact staged file state from before the rewrite pass
            restore_file_overrides(&mut collected.files, &snapshots);
            eprintln!(
                "preset export warning: leaving host-specific CSS asset references unchanged in the bundle"
            );
            Ok(Vec::new())
        }
        Err(err) => {
            // Prompt failures must not leak half-rewritten staged bytes into later export logic
            restore_file_overrides(&mut collected.files, &snapshots);
            Err(err)
        }
    }
}

pub(in crate::preset::export) fn rewrite_host_specific_script_paths_if_requested<G>(
    config_dir: &Path,
    collected: &mut CollectedConfigFiles,
    prompt_fix_host_specific_script_paths: G,
) -> Result<Vec<HostSpecificScriptLeak>>
where
    G: FnOnce(&[HostSpecificScriptLeak]) -> Result<bool>,
{
    let snapshots = capture_file_overrides(&collected.files);

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

    match prompt_fix_host_specific_script_paths(&leaked_refs) {
        Ok(true) => {
            // Keep staged script rewrites
            Ok(leaked_refs)
        }
        Ok(false) => {
            // Declining rewrite keeps the original script bytes and size in the staged archive
            restore_file_overrides(&mut collected.files, &snapshots);
            eprintln!(
                "preset export warning: leaving host-specific script path references unchanged in the bundle"
            );
            Ok(Vec::new())
        }
        Err(err) => {
            // Prompt failures must roll back staged rewrites too
            restore_file_overrides(&mut collected.files, &snapshots);
            Err(err)
        }
    }
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
            let matched = leak.needles.join(", ");
            // Include replacement text to show final form
            format!(
                "  - {} contains {} (let noticenterctl rewrite it to {})",
                leak.script_path.display(),
                matched,
                leak.rewritten_to
            )
        })
        .collect()
}

#[cfg(test)]
#[path = "tests/rewrite.rs"]
mod tests;
