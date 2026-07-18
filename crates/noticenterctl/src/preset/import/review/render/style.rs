//! Terminal color policy and styled review output

use std::env;
use std::fmt;
use std::io::{self, IsTerminal};

#[derive(Clone, Copy)]
pub(super) enum ReviewTone {
    Title,
    Warning,
    Note,
    Section,
    Slot,
    Command,
    FileHeader,
    FileBody,
}

impl ReviewTone {
    const fn ansi_prefix(self) -> &'static str {
        match self {
            Self::Title => "1;36",
            Self::Warning => "1;33",
            Self::Note => "2",
            Self::Section => "1;35",
            Self::Slot => "1;34",
            Self::Command => "32",
            Self::FileHeader => "1;33",
            Self::FileBody => "37",
        }
    }
}

pub(super) fn write_styled<Output, Body>(
    output: &mut Output,
    style: ReviewStyle,
    tone: ReviewTone,
    write_body: Body,
) -> fmt::Result
where
    Output: fmt::Write + ?Sized,
    Body: FnOnce(&mut Output) -> fmt::Result,
{
    // Styling wraps the streamed body without allocating a second string
    if style.color {
        write!(output, "\u{1b}[{}m", tone.ansi_prefix())?;
    }
    write_body(output)?;
    if style.color {
        output.write_str("\u{1b}[0m")?;
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::preset) struct ReviewStyle {
    pub(in crate::preset) color: bool,
}

impl ReviewStyle {
    pub(in crate::preset) fn for_terminal() -> Self {
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
}
