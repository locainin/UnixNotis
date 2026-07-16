//! Journal terminal sanitization and presentation limits

use super::super::report::{redact_home_text, safe_doctor_text, truncate_with_ellipsis};

// Journal limits bound both report size and time spent processing untrusted service output
pub(super) const JOURNAL_LINE_LIMIT: usize = 30;
pub(super) const JOURNAL_TOTAL_BYTE_LIMIT: usize = 32 * 1024;
pub(super) const JOURNAL_LINE_CHAR_LIMIT: usize = 512;

#[derive(Debug)]
pub(super) struct JournalCollection {
    pub(super) lines: Vec<String>,
    // Content truncation covers line-count and per-line character limits
    pub(super) content_truncated: bool,
    // Byte truncation is recorded by the bounded process reader
    pub(super) byte_truncated: bool,
}

impl JournalCollection {
    pub(super) const fn was_truncated(&self) -> bool {
        self.content_truncated || self.byte_truncated
    }
}

pub(super) const fn journal_output_exceeds_limit(byte_count: usize) -> bool {
    byte_count > JOURNAL_TOTAL_BYTE_LIMIT
}

pub(super) fn sanitize_journal(bytes: &[u8]) -> JournalCollection {
    // Apply line and character caps after terminal-control sanitization
    let text = String::from_utf8_lossy(bytes);
    // Count before take() so a clipped line window remains visible in structured output
    let source_line_count = text.lines().count();
    let mut line_truncated = false;
    let lines = text
        .lines()
        .take(JOURNAL_LINE_LIMIT)
        .map(safe_doctor_text)
        .map(|line| redact_home_text(&line))
        .map(|line| {
            if line.chars().count() > JOURNAL_LINE_CHAR_LIMIT {
                // One clipped line is enough to mark the entire collection as partial
                line_truncated = true;
            }
            truncate_with_ellipsis(&line, JOURNAL_LINE_CHAR_LIMIT)
        })
        .collect();
    JournalCollection {
        lines,
        content_truncated: line_truncated || source_line_count > JOURNAL_LINE_LIMIT,
        // The command reader sets this when stdout exceeds the byte window
        byte_truncated: false,
    }
}
