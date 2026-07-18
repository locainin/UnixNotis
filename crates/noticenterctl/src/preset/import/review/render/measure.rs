//! Checked byte measurement for review output

use std::fmt;

use super::super::checks::ImportedExecContent;
use super::content::write_exec_review;
use super::model::ReviewDetail;
use super::style::ReviewStyle;

pub(super) fn measured_review_size(
    exec_content: &ImportedExecContent,
    style: ReviewStyle,
    detail: ReviewDetail,
    complete: bool,
) -> Option<usize> {
    let mut counter = CheckedByteCounter::default();
    // The shared writer makes measured bytes match the eventual rendered representation
    write_exec_review(&mut counter, exec_content, style, detail, complete).ok()?;
    Some(counter.bytes)
}

#[derive(Default)]
struct CheckedByteCounter {
    bytes: usize,
}

impl fmt::Write for CheckedByteCounter {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        // Checked addition makes even an impossible platform-sized review fail closed
        self.bytes = self.bytes.checked_add(value.len()).ok_or(fmt::Error)?;
        Ok(())
    }
}
