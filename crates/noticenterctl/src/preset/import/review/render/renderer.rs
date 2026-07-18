//! Aggregate limit selection and final review allocation

use super::super::checks::ImportedExecContent;
use super::content::write_exec_review;
use super::measure::measured_review_size;
use super::model::{
    RenderedExecReview, ReviewDetail, MAX_COMPLETE_REVIEW_COMMAND_BYTES,
    MAX_COMPLETE_REVIEW_OUTPUT_BYTES, MAX_COMPLETE_REVIEW_TEXT_BYTES,
};
use super::style::ReviewStyle;

pub(in crate::preset) fn render_exec_content_review_with_style(
    exec_content: &ImportedExecContent,
    style: ReviewStyle,
) -> RenderedExecReview {
    let items_complete = item_review_is_complete(exec_content);
    // Count escaped bytes before constructing them so control-heavy input cannot amplify memory use
    let full_size = measured_review_size(exec_content, style, ReviewDetail::Full, items_complete);
    let aggregate_limited = full_size.is_none_or(|size| size > MAX_COMPLETE_REVIEW_OUTPUT_BYTES);
    let complete = items_complete && !aggregate_limited;

    let detail = if aggregate_limited {
        // Metadata keeps every command slot and file identity visible when their bodies are too large together
        let metadata_size =
            measured_review_size(exec_content, style, ReviewDetail::Metadata, false);
        if metadata_size.is_some_and(|size| size <= MAX_COMPLETE_REVIEW_OUTPUT_BYTES) {
            ReviewDetail::Metadata
        } else {
            ReviewDetail::Summary
        }
    } else {
        ReviewDetail::Full
    };

    let rendered_size = measured_review_size(exec_content, style, detail, complete)
        .unwrap_or(MAX_COMPLETE_REVIEW_OUTPUT_BYTES)
        .min(MAX_COMPLETE_REVIEW_OUTPUT_BYTES);
    let mut rendered = String::with_capacity(rendered_size);
    // String's formatter is infallible, while the shared formatter keeps both passes identical
    if write_exec_review(&mut rendered, exec_content, style, detail, complete).is_err() {
        rendered.clear();
        rendered.push_str("Review status: incomplete; executable review could not be rendered\n");
        return RenderedExecReview {
            rendered,
            complete: false,
        };
    }

    RenderedExecReview { rendered, complete }
}

fn item_review_is_complete(exec_content: &ImportedExecContent) -> bool {
    let commands_fit = exec_content
        .commands
        .iter()
        .all(|command| command.command.len() <= MAX_COMPLETE_REVIEW_COMMAND_BYTES);
    let text_files_fit = exec_content.files.iter().all(|file| {
        std::str::from_utf8(&file.contents).map_or(true, |_| {
            file.contents.len() <= MAX_COMPLETE_REVIEW_TEXT_BYTES
        })
    });
    commands_fit && text_files_fit
}
