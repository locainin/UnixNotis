#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(in super::super) enum CssCheckSeverity {
    // Errors should always stand out first in the final report
    Error,
    // Warnings keep the run successful but still need attention
    Warning,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(in super::super) enum CssCheckCategory {
    // Parser failures come from GTK itself
    Parse,
    // Theme warnings come from file paths and shareability checks
    Theme,
    // Runtime warnings come from config values that the app clamps later
    Runtime,
    // Lint warnings come from css text that looks risky or redundant
    Lint,
    // Geometry warnings come from estimated layout pressure
    Geometry,
}

impl CssCheckCategory {
    pub(in super::super) fn label(self) -> &'static str {
        match self {
            Self::Parse => "parse",
            Self::Theme => "theme",
            Self::Runtime => "runtime",
            Self::Lint => "lint",
            Self::Geometry => "geometry",
        }
    }

    pub(in super::super) fn summary_label(self) -> &'static str {
        match self {
            Self::Parse => "parse",
            Self::Theme => "theme",
            Self::Runtime => "runtime",
            Self::Lint => "lint",
            Self::Geometry => "geometry",
        }
    }

    pub(in super::super) fn sort_rank(self) -> u8 {
        match self {
            Self::Parse => 0,
            Self::Theme => 1,
            Self::Runtime => 2,
            Self::Lint => 3,
            Self::Geometry => 4,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(in super::super) struct CssCheckDiagnostic {
    // Severity decides both ordering and final result color
    pub(in super::super) severity: CssCheckSeverity,
    // Category keeps related warnings grouped together
    pub(in super::super) category: CssCheckCategory,
    // Stable codes make docs and tests easier to pin down
    pub(in super::super) code: &'static str,
    // This is already formatted for terminal display
    pub(in super::super) display_path: String,
    // Source locations show up when lint or parsing can point at a real spot
    pub(in super::super) line: Option<usize>,
    pub(in super::super) column: Option<usize>,
    pub(in super::super) message: String,
    // Hints stay optional so short warnings do not get bloated
    pub(in super::super) hint: Option<String>,
}

impl CssCheckDiagnostic {
    pub(in super::super) fn warning(
        category: CssCheckCategory,
        code: &'static str,
        display_path: String,
        message: impl Into<String>,
    ) -> Self {
        // Most warnings still read cleanly without a source location
        Self::warning_at(category, code, display_path, None, None, message)
    }

    pub(in super::super) fn warning_at(
        category: CssCheckCategory,
        code: &'static str,
        display_path: String,
        line: Option<usize>,
        column: Option<usize>,
        message: impl Into<String>,
    ) -> Self {
        // Most warnings still have no source location, so the fields stay optional
        Self {
            severity: CssCheckSeverity::Warning,
            category,
            code,
            display_path,
            line,
            column,
            message: message.into(),
            hint: None,
        }
    }

    pub(in super::super) fn error(
        category: CssCheckCategory,
        code: &'static str,
        display_path: String,
        line: Option<usize>,
        column: Option<usize>,
        message: impl Into<String>,
        hint: Option<String>,
    ) -> Self {
        // Errors keep the extra source context when it exists
        Self {
            severity: CssCheckSeverity::Error,
            category,
            code,
            display_path,
            line,
            column,
            message: message.into(),
            hint,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(in super::super) struct CssCheckActiveFile {
    // The slot name shows which config key resolved to this file
    pub(in super::super) slot_name: &'static str,
    pub(in super::super) display_path: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(in super::super) struct CssCheckReport {
    // The displayed root may be $HOME or $XDG_CONFIG_HOME based
    pub(in super::super) display_root: String,
    pub(in super::super) checked_files: usize,
    // Active theme files are shown when the report needs more context
    pub(in super::super) active_files: Vec<CssCheckActiveFile>,
    // Notes are extra context that should not count as warnings
    pub(in super::super) notes: Vec<String>,
    pub(in super::super) diagnostics: Vec<CssCheckDiagnostic>,
}

impl CssCheckReport {
    pub(in super::super) fn error_count(&self) -> usize {
        // Errors decide whether css-check returns a failure
        self.diagnostics
            .iter()
            .filter(|item| item.severity == CssCheckSeverity::Error)
            .count()
    }

    pub(in super::super) fn warning_count(&self) -> usize {
        // Warnings keep the run successful but change the final verdict
        self.diagnostics
            .iter()
            .filter(|item| item.severity == CssCheckSeverity::Warning)
            .count()
    }

    pub(in super::super) fn is_clean(&self) -> bool {
        // A clean report means no diagnostics and no extra notes
        self.diagnostics.is_empty() && self.notes.is_empty()
    }
}
