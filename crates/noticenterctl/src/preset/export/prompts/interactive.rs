//! Interactive export confirmations and non-interactive refusal

use std::io::IsTerminal;

use anyhow::{anyhow, Result};

use crate::preset::command_rules::HostSpecificCommandPath;
use crate::preset::css_asset_refs::{ExternalCssAssetRef, HostSpecificCssAssetRef};
use crate::preset::export::checks::HostSpecificScriptLeak;
use crate::preset::pathing::{confirm_continue_or_abort, prompt_yes_no};

pub(in crate::preset::export) fn confirm_export_external_css_refs(
    external_refs: &[ExternalCssAssetRef],
) -> Result<()> {
    if external_refs.is_empty() {
        // Portable stylesheets need no confirmation and stay automation-friendly
        return Ok(());
    }

    // Print every dependency before asking whether a non-portable export should continue
    let details = format_external_css_ref_lines(external_refs);
    eprintln!(
        "preset export warning: found {} CSS asset reference(s) that leave the UnixNotis config directory or use remote URLs",
        external_refs.len()
    );
    for line in &details {
        eprintln!("{line}");
    }

    confirm_continue_or_abort(
        "External CSS asset references were found; continue exporting anyway?",
        &format!(
            "preset export found CSS asset references that leave the UnixNotis config directory or use remote URLs; rerun interactively to confirm anyway\n{}",
            details.join("\n")
        ),
    )
}

pub(in crate::preset::export) fn prompt_to_fix_host_specific_command_paths(
    _leaked_paths: &[HostSpecificCommandPath],
) -> Result<bool> {
    if std::io::stdin().is_terminal() && std::io::stdout().is_terminal() {
        // Rewriting requires an attached input and output terminal for explicit consent
        return prompt_yes_no(
            "Host-specific command paths were found; let noticenterctl rewrite them in the exported preset?",
        );
    }

    // Non-interactive execution cannot guess whether staged content may be rewritten
    Err(anyhow!(
        "preset export found host-specific command paths under the UnixNotis config directory; rerun interactively to let noticenterctl rewrite them"
    ))
}

pub(in crate::preset::export) fn prompt_to_fix_host_specific_css_asset_refs(
    _leaked_refs: &[HostSpecificCssAssetRef],
) -> Result<bool> {
    if std::io::stdin().is_terminal() && std::io::stdout().is_terminal() {
        // CSS references are rewritten only after the operator sees the portability warning
        return prompt_yes_no(
            "Host-specific CSS asset references were found; let noticenterctl rewrite them in the exported preset?",
        );
    }

    Err(anyhow!(
        "preset export found host-specific CSS asset references under the UnixNotis config directory; rerun interactively to let noticenterctl rewrite them"
    ))
}

pub(in crate::preset::export) fn prompt_to_fix_host_specific_script_paths(
    _leaked_refs: &[HostSpecificScriptLeak],
) -> Result<bool> {
    if std::io::stdin().is_terminal() && std::io::stdout().is_terminal() {
        // Script contents can carry meaningful paths so non-interactive guessing is forbidden
        return prompt_yes_no(
            "Host-specific script paths were found; let noticenterctl rewrite them in the exported preset?",
        );
    }

    Err(anyhow!(
        "preset export found host-specific script path references under the UnixNotis config directory; rerun interactively to let noticenterctl rewrite them"
    ))
}

fn format_external_css_ref_lines(external_refs: &[ExternalCssAssetRef]) -> Vec<String> {
    // Stable one-line entries keep terminal and captured CI output easy to compare
    external_refs
        .iter()
        .map(|asset_ref| {
            let detail = if asset_ref.reason == "remote url" {
                // Present the internal classifier in normal user-facing capitalization
                "remote URL".to_string()
            } else {
                asset_ref.reason.clone()
            };
            format!(
                "  - {} -> {} ({})",
                asset_ref.css_file.display(),
                asset_ref.asset_ref,
                detail
            )
        })
        .collect()
}

#[cfg(test)]
#[path = "tests/interactive.rs"]
mod tests;
