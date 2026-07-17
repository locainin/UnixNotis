//! Interactive import warning prompts

use anyhow::Result;

use super::super::super::css_asset_refs::ExternalCssAssetRef;
use super::super::super::pathing::{
    confirm_continue_or_abort_with_terminal_state, terminal_interaction_available,
};

pub(in crate::preset) fn confirm_import_external_css_refs(
    external_refs: &[ExternalCssAssetRef],
) -> Result<()> {
    confirm_import_external_css_refs_with_terminal_state(
        external_refs,
        terminal_interaction_available(),
    )
}

pub(in crate::preset) fn confirm_import_external_css_refs_with_terminal_state(
    external_refs: &[ExternalCssAssetRef],
    terminal_interactive: bool,
) -> Result<()> {
    if external_refs.is_empty() {
        return Ok(());
    }

    // The caller needs the concrete file and ref before deciding whether portability matters here
    let details = format_external_css_ref_lines(external_refs);
    eprintln!(
        "preset import warning: found {} CSS asset reference(s) that leave the UnixNotis config directory or use remote URLs",
        external_refs.len()
    );
    for line in &details {
        eprintln!("{line}");
    }

    confirm_continue_or_abort_with_terminal_state(
        "External CSS asset references were found; continue importing anyway?",
        &format!(
            "preset import found CSS asset references that leave the UnixNotis config directory or use remote URLs; rerun interactively to confirm anyway\n{}",
            details.join("\n")
        ),
        terminal_interactive,
    )
}

pub(in crate::preset) fn format_external_css_ref_lines(
    external_refs: &[ExternalCssAssetRef],
) -> Vec<String> {
    external_refs
        .iter()
        .map(|asset_ref| {
            let detail = if asset_ref.reason == "remote url" {
                "remote URL".to_string()
            } else {
                asset_ref.reason.clone()
            };
            // One-line rows make the warning easy to scan before the prompt is shown
            format!(
                "  - {} -> {} ({})",
                asset_ref.css_file.display(),
                asset_ref.asset_ref,
                detail
            )
        })
        .collect()
}
