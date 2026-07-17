//! Interactive review for executable preset content
//!
//! Human-driven imports should get a chance to inspect risky content before trusting it

use anyhow::{anyhow, Context, Result};
use std::env;
use std::io::{self, IsTerminal, Write};
use unixnotis_core::util;

use super::super::super::pathing::{prompt_yes_no, terminal_interaction_available};
use super::checks::ImportedExecContent;
use super::pager::page_exec_content_review;

const MAX_REVIEW_COMMANDS: usize = 64;
const MAX_REVIEW_FILES: usize = 32;
const MAX_REVIEW_INLINE_CHARS: usize = 512;
const MAX_REVIEW_FILE_CHARS: usize = 4_096;

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
         preset import warning: found {} command entr{} and {} bundled file{} with executable content\n",
        exec_content.commands.len(),
        if exec_content.commands.len() == 1 { "y" } else { "ies" },
        exec_content.files.len(),
        if exec_content.files.len() == 1 { "" } else { "s" }
    ))?;

    // Review happens before the final trust prompt so the decision is made with context
    if prompt_yes_no("Inspect executable content now?")? {
        let review =
            render_exec_content_review_with_style(exec_content, ReviewStyle::for_terminal());
        if !page_exec_content_review(&review)? {
            // Minimal systems still receive the full review when no trusted pager is installed
            let stderr = io::stderr();
            let mut stderr = stderr.lock();
            write_exec_content_review(&mut stderr, &review)?;
        }
    }

    // A second prompt keeps pager exit from being treated as implicit approval
    if prompt_yes_no("Import this preset anyway?")? {
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

pub(in crate::preset) fn render_exec_content_review_with_style(
    exec_content: &ImportedExecContent,
    style: ReviewStyle,
) -> String {
    let mut lines = vec![
        style.title("UnixNotis preset executable content review"),
        String::new(),
        style.warning("This preset contains executable commands or bundled scripts"),
        style.note("Only continue if the source is trusted"),
    ];

    if !exec_content.commands.is_empty() {
        lines.push(String::new());
        lines.push(style.section("Command entries"));
        for command in exec_content.commands.iter().take(MAX_REVIEW_COMMANDS) {
            // Slot names make it obvious which config field would become runnable
            lines.push(format!(
                "  {} = {}",
                style.slot(safe_review_inline(&command.slot)),
                style.command(safe_review_inline(&command.command))
            ));
        }
        append_omitted_count(
            &mut lines,
            style,
            exec_content.commands.len(),
            MAX_REVIEW_COMMANDS,
            "command entries",
        );
    }

    if !exec_content.files.is_empty() {
        lines.push(String::new());
        lines.push(style.section("Bundled executable files"));
        for file in exec_content.files.iter().take(MAX_REVIEW_FILES) {
            lines.push(String::new());
            lines.push(style.file_header(format!(
                "== {} (mode {:o}) ==",
                safe_review_inline(&file.relative_path.display().to_string()),
                file.mode
            )));
            match std::str::from_utf8(&file.contents) {
                // Text payloads are shown directly so the trust check can happen without unpacking
                Ok(text) => lines.push(style.file_body(util::sanitize_display_text_bounded(
                    text,
                    MAX_REVIEW_FILE_CHARS,
                ))),
                Err(_) => lines.push(style.note(format!(
                    "<non-UTF-8 file omitted; {} byte(s)>",
                    file.contents.len()
                ))),
            }
        }
        append_omitted_count(
            &mut lines,
            style,
            exec_content.files.len(),
            MAX_REVIEW_FILES,
            "executable files",
        );
    }

    lines.push(String::new());
    lines.join("\n")
}

fn safe_review_inline(value: &str) -> String {
    // Commands and paths stay on one row so embedded controls cannot rewrite the prompt
    util::sanitize_log_value(value, MAX_REVIEW_INLINE_CHARS)
}

fn append_omitted_count(
    lines: &mut Vec<String>,
    style: ReviewStyle,
    total: usize,
    shown: usize,
    label: &str,
) {
    let omitted = total.saturating_sub(shown);
    if omitted > 0 {
        // The summary makes a bounded review explicit instead of implying every payload was shown
        lines.push(style.note(format!("<{omitted} additional {label} omitted>")));
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::preset) struct ReviewStyle {
    pub(in crate::preset) color: bool,
}

impl ReviewStyle {
    fn for_terminal() -> Self {
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
