//! Interactive review for executable preset content
//!
//! Human-driven imports should get a chance to inspect risky content before trusting it

use anyhow::{anyhow, Context, Result};
use std::env;
use std::io::{self, IsTerminal, Write};

use super::super::super::pathing::{prompt_yes_no, terminal_interaction_available};
use super::checks::ImportedExecContent;

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

    eprintln!("preset import warning: this preset contains executable commands or bundled scripts");
    eprintln!("preset import warning: be sure the source is trusted");
    eprintln!(
        "preset import warning: found {} command entr{} and {} bundled file{} with executable content",
        exec_content.commands.len(),
        if exec_content.commands.len() == 1 { "y" } else { "ies" },
        exec_content.files.len(),
        if exec_content.files.len() == 1 { "" } else { "s" }
    );

    // Review happens before the final trust prompt so the decision is made with context
    if prompt_yes_no("Inspect executable content now?")? {
        let review =
            render_exec_content_review_with_style(exec_content, ReviewStyle::for_terminal());
        let stderr = io::stderr();
        let mut stderr = stderr.lock();
        write_exec_content_review(&mut stderr, &review)?;
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
    // Security review text is written directly; no pager or shell can run while trust is undecided
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
        for command in &exec_content.commands {
            // Slot names make it obvious which config field would become runnable
            lines.push(format!(
                "  {} = {}",
                style.slot(&command.slot),
                style.command(&command.command)
            ));
        }
    }

    if !exec_content.files.is_empty() {
        lines.push(String::new());
        lines.push(style.section("Bundled executable files"));
        for file in &exec_content.files {
            lines.push(String::new());
            lines.push(style.file_header(format!(
                "== {} (mode {:o}) ==",
                file.relative_path.display(),
                file.mode
            )));
            match std::str::from_utf8(&file.contents) {
                // Text payloads are shown directly so the trust check can happen without unpacking
                Ok(text) => lines.push(style.file_body(text)),
                Err(_) => lines.push(style.note(format!(
                    "<non-UTF-8 file omitted; {} byte(s)>",
                    file.contents.len()
                ))),
            }
        }
    }

    lines.push(String::new());
    lines.join("\n")
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::preset) struct ReviewStyle {
    pub(in crate::preset) color: bool,
}

impl ReviewStyle {
    fn for_terminal() -> Self {
        // Pager review is only useful with color on real terminals
        let color = io::stdout().is_terminal()
            && env::var_os("NO_COLOR").is_none()
            && env::var("CLICOLOR").map_or(true, |value| value != "0")
            && env::var("TERM").map_or(true, |value| value != "dumb");
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
