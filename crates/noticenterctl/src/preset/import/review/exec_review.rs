//! Interactive review for executable preset content
//!
//! Interactive imports require a complete review before approval

use anyhow::{anyhow, Context, Result};
use std::io::{self, Write};

use super::super::super::pathing::{prompt_yes_no, terminal_interaction_available};
use super::checks::ImportedExecContent;
use super::pager::page_exec_content_review;
use super::render::{render_exec_content_review_with_style, RenderedExecReview, ReviewStyle};

pub(in crate::preset) fn confirm_import_exec_content(
    exec_content: &ImportedExecContent,
    allow_exec: bool,
) -> Result<()> {
    confirm_import_exec_content_with_terminal_state(
        exec_content,
        allow_exec,
        terminal_interaction_available(),
    )
}

pub(in crate::preset) fn confirm_import_exec_content_with_terminal_state(
    exec_content: &ImportedExecContent,
    allow_exec: bool,
    terminal_interactive: bool,
) -> Result<()> {
    confirm_import_exec_content_with_interaction(
        exec_content,
        allow_exec,
        terminal_interactive,
        prompt_yes_no,
        |review| {
            if page_exec_content_review(&review.rendered)? {
                return Ok(());
            }

            // Minimal systems still receive the complete bounded review when less is unavailable
            let stderr = io::stderr();
            let mut stderr = stderr.lock();
            write_exec_content_review(&mut stderr, &review.rendered)
        },
    )
}

pub(in crate::preset) fn confirm_import_exec_content_with_interaction(
    exec_content: &ImportedExecContent,
    allow_exec: bool,
    terminal_interactive: bool,
    mut prompt: impl FnMut(&str) -> Result<bool>,
    show_review: impl FnOnce(&RenderedExecReview) -> Result<()>,
) -> Result<()> {
    // Empty bundles stay on the normal import path without extra prompts
    if exec_content.commands.is_empty() && exec_content.files.is_empty() {
        return Ok(());
    }

    // Explicit trust should keep automation and existing scripted flows working
    if allow_exec {
        return Ok(());
    }

    if !terminal_interactive {
        return Err(anyhow!(
            "preset import found executable commands or bundled scripts; rerun interactively to inspect them or use --allow-exec only if the preset is trusted"
        ));
    }

    crate::output::write_stderr(&format!(
        "preset import warning: this preset contains executable commands or bundled scripts\n\
         preset import warning: be sure the source is trusted\n\
         preset import warning: found {} command entr{} and {} bundled file{} requiring review\n",
        exec_content.commands.len(),
        if exec_content.commands.len() == 1 {
            "y"
        } else {
            "ies"
        },
        exec_content.files.len(),
        if exec_content.files.len() == 1 {
            ""
        } else {
            "s"
        }
    ))?;

    // Review is required because the final approval depends on seeing the actual imported content
    if !prompt("Inspect executable content now?")? {
        return Err(anyhow!(
            "preset command canceled because executable content review is required"
        ));
    }

    let review = render_exec_content_review_with_style(exec_content, ReviewStyle::for_terminal());
    show_review(&review)?;
    ensure_exec_review_complete(&review)?;

    // A second prompt keeps pager exit from being treated as implicit approval
    if prompt("Import this preset anyway?")? {
        return Ok(());
    }

    Err(anyhow!("preset command canceled"))
}

pub(in crate::preset) fn write_exec_content_review(
    writer: &mut impl Write,
    review: &str,
) -> Result<()> {
    // This path is also the safe fallback when the trusted pager is unavailable
    writer
        .write_all(review.as_bytes())
        .context("write executable content review")?;
    writer.flush().context("flush executable content review")
}

pub(in crate::preset) fn ensure_exec_review_complete(review: &RenderedExecReview) -> Result<()> {
    if review.complete {
        return Ok(());
    }

    Err(anyhow!(
        "preset import review is incomplete; inspect the bundle independently and rerun with --allow-exec only if it is trusted"
    ))
}
