//! Review limits and rendering state

// Individual bodies stay small enough for deliberate line-by-line inspection
pub(super) const MAX_COMPLETE_REVIEW_COMMAND_BYTES: usize = 64 * 1_024;
pub(super) const MAX_COMPLETE_REVIEW_TEXT_BYTES: usize = 64 * 1_024;
// Aggregate output is measured before allocation to cap expansion across all items
pub(in crate::preset) const MAX_COMPLETE_REVIEW_OUTPUT_BYTES: usize = 8 * 1_024 * 1_024;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::preset) struct RenderedExecReview {
    // The pager receives this complete terminal-safe representation
    pub(in crate::preset) rendered: String,
    // Ordinary approval is available only when no command or text payload was skipped
    pub(in crate::preset) complete: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ReviewDetail {
    // Full detail retains every item that fits its own review limit
    Full,
    // Metadata detail avoids expanding command and file bodies
    Metadata,
    // Summary detail handles the unlikely case where metadata alone exceeds the output limit
    Summary,
}
