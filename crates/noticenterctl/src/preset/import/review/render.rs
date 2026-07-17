//! Terminal-safe rendering for executable preset content

use std::env;
use std::fmt::Write as _;
use std::io::{self, IsTerminal};

use super::checks::ImportedExecContent;

const MAX_COMPLETE_REVIEW_COMMAND_BYTES: usize = 64 * 1_024;
const MAX_COMPLETE_REVIEW_TEXT_BYTES: usize = 64 * 1_024;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::preset) struct RenderedExecReview {
    // The pager receives this complete terminal-safe representation
    pub(in crate::preset) rendered: String,
    // Ordinary approval is available only when no command or text payload was skipped
    pub(in crate::preset) complete: bool,
}

pub(in crate::preset) fn render_exec_content_review_with_style(
    exec_content: &ImportedExecContent,
    style: ReviewStyle,
) -> RenderedExecReview {
    let mut complete = true;
    let mut lines = vec![
        style.title("UnixNotis preset executable content review"),
        String::new(),
        style.warning("This preset contains executable commands or bundled scripts"),
        style.note("Only continue if the source is trusted"),
    ];

    if !exec_content.commands.is_empty() {
        lines.push(String::new());
        lines.push(style.section("Command entries"));
        for command in &exec_content.commands {
            // Full command text is required because meaningful work may be hidden at the end
            let displayed_command = if command.command.len() <= MAX_COMPLETE_REVIEW_COMMAND_BYTES {
                style.command(escape_review_inline(&command.command))
            } else {
                complete = false;
                style.note(format!(
                    "<command not displayed; {} bytes; BLAKE3 {}>",
                    command.command.len(),
                    blake3::hash(command.command.as_bytes()).to_hex()
                ))
            };
            lines.push(format!(
                "  {} = {}",
                style.slot(escape_review_inline(&command.slot)),
                displayed_command
            ));
        }
    }

    if !exec_content.files.is_empty() {
        lines.push(String::new());
        lines.push(style.section("Bundled files available to commands"));
        for file in &exec_content.files {
            let digest = blake3::hash(&file.contents);
            lines.push(String::new());
            lines.push(style.file_header(format!(
                "== {} (mode {:o}, {} bytes, BLAKE3 {}) ==",
                escape_review_inline(&file.relative_path.display().to_string()),
                file.mode,
                file.contents.len(),
                digest.to_hex()
            )));
            match std::str::from_utf8(&file.contents) {
                // Small text files are represented completely with unsafe controls escaped
                Ok(text) if file.contents.len() <= MAX_COMPLETE_REVIEW_TEXT_BYTES => {
                    lines.push(style.file_body(escape_review_text(text)));
                }
                Ok(_) => {
                    complete = false;
                    lines.push(style.note(format!(
                        "<text not displayed because it exceeds {MAX_COMPLETE_REVIEW_TEXT_BYTES} bytes>"
                    )));
                }
                // Binary content is represented by its exact size and digest in the header
                Err(_) => lines.push(style.note("<binary content represented by metadata above>")),
            }
        }
    }

    let status = if complete {
        style.note("Review status: complete")
    } else {
        style.warning("Review status: incomplete; ordinary approval is disabled")
    };
    lines.insert(4, String::new());
    lines.insert(5, status);
    lines.push(String::new());
    RenderedExecReview {
        rendered: lines.join("\n"),
        complete,
    }
}

fn escape_review_inline(value: &str) -> String {
    escape_review_text_with(value, false)
}

fn escape_review_text(value: &str) -> String {
    escape_review_text_with(value, true)
}

fn escape_review_text_with(value: &str, keep_newlines: bool) -> String {
    let mut escaped = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '\n' if keep_newlines => escaped.push('\n'),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            '\\' => escaped.push_str("\\\\"),
            _ if ch.is_control() || is_bidi_control(ch) => {
                // Visible escapes preserve every character without allowing terminal effects
                let _ = write!(escaped, "\\u{{{:04x}}}", u32::from(ch));
            }
            _ => escaped.push(ch),
        }
    }
    escaped
}

const fn is_bidi_control(ch: char) -> bool {
    matches!(
        ch,
        '\u{061C}'
            | '\u{200E}'
            | '\u{200F}'
            | '\u{202A}'
            | '\u{202B}'
            | '\u{202C}'
            | '\u{202D}'
            | '\u{202E}'
            | '\u{2066}'
            | '\u{2067}'
            | '\u{2068}'
            | '\u{2069}'
    )
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::preset) struct ReviewStyle {
    pub(in crate::preset) color: bool,
}

impl ReviewStyle {
    pub(super) fn for_terminal() -> Self {
        // Pager review is only useful with color on real terminals
        Self::for_terminal_state(
            io::stdout().is_terminal(),
            env::var_os("NO_COLOR").is_some(),
            env::var("CLICOLOR").ok().as_deref(),
            env::var("TERM").ok().as_deref(),
        )
    }

    pub(in crate::preset) fn for_terminal_state(
        terminal: bool,
        no_color: bool,
        clicolor: Option<&str>,
        term: Option<&str>,
    ) -> Self {
        // Every opt-out is independent so redirected and accessibility-friendly output stays plain
        let color = terminal && !no_color && clicolor != Some("0") && term != Some("dumb");
        Self { color }
    }

    fn paint(self, text: impl Into<String>, prefix: &str) -> String {
        let text = text.into();
        if !self.color {
            // Plain output keeps the review readable when color is disabled upstream
            return text;
        }
        format!("\u{1b}[{prefix}m{text}\u{1b}[0m")
    }

    pub(in crate::preset) fn title(self, text: impl Into<String>) -> String {
        self.paint(text, "1;36")
    }

    fn warning(self, text: impl Into<String>) -> String {
        self.paint(text, "1;33")
    }

    fn note(self, text: impl Into<String>) -> String {
        self.paint(text, "2")
    }

    fn section(self, text: impl Into<String>) -> String {
        self.paint(text, "1;35")
    }

    fn slot(self, text: impl Into<String>) -> String {
        self.paint(text, "1;34")
    }

    fn command(self, text: impl Into<String>) -> String {
        self.paint(text, "32")
    }

    fn file_header(self, text: impl Into<String>) -> String {
        self.paint(text, "1;33")
    }

    fn file_body(self, text: impl Into<String>) -> String {
        self.paint(text, "37")
    }
}
