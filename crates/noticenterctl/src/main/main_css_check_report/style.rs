use std::env;
use std::io::{self, IsTerminal};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct ReportStyle {
    // One flag is enough because all colors are derived from the same choice
    pub(super) color: bool,
}

impl ReportStyle {
    pub(super) fn for_stdout() -> Self {
        // Color should stay off for pipes, NO_COLOR, or dumb terminals
        let color = io::stdout().is_terminal()
            && env::var_os("NO_COLOR").is_none()
            && env::var("CLICOLOR")
                .map(|value| value != "0")
                .unwrap_or(true)
            && env::var("TERM")
                .map(|value| value != "dumb")
                .unwrap_or(true);
        Self { color }
    }

    pub(super) fn paint(self, text: impl Into<String>, prefix: &str) -> String {
        let text = text.into();
        if !self.color {
            // Plain output keeps tests and redirected output stable
            return text;
        }

        // One reset at the end keeps nested formatting simple
        format!("\u{1b}[{prefix}m{text}\u{1b}[0m")
    }

    pub(super) fn bold(self, text: impl Into<String>) -> String {
        // Bold is used for headers and file paths only
        self.paint(text, "1")
    }

    pub(super) fn errors_title(self, text: impl Into<String>) -> String {
        // Error headers should stand out immediately
        self.paint(text, "1;31")
    }

    pub(super) fn warnings_title(self, text: impl Into<String>) -> String {
        // Warning headers use yellow to stay obvious without looking fatal
        self.paint(text, "1;33")
    }

    pub(super) fn notes_title(self, text: impl Into<String>) -> String {
        // Notes use a cooler color so they read as context instead of alarm
        self.paint(text, "1;36")
    }

    pub(super) fn categories_title(self, text: impl Into<String>) -> String {
        // Category labels use a different accent so the summary does not blend into warnings
        self.paint(text, "1;35")
    }

    pub(super) fn result(self, text: impl Into<String>, errors: usize, warnings: usize) -> String {
        // The final verdict color follows the same rules as the real command status
        if errors > 0 {
            return self.paint(text, "1;31");
        }
        if warnings > 0 {
            return self.paint(text, "1;33");
        }
        self.paint(text, "1;32")
    }

    pub(super) fn diagnostic_code(self, text: impl Into<String>) -> String {
        // Codes stay dim so the warning text stays easier to read than the prefix
        self.paint(text, "2;35")
    }
}
