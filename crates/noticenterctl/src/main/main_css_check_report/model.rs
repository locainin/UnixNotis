#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum CssCheckSeverity {
    // Errors should always stand out first in the final report
    Error,
    // Warnings keep the run successful but still need attention
    Warning,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum CssCheckCategory {
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
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Parse => "parse",
            Self::Theme => "theme",
            Self::Runtime => "runtime",
            Self::Lint => "lint",
            Self::Geometry => "geometry",
        }
    }

    pub(crate) fn summary_label(self) -> &'static str {
        match self {
            Self::Parse => "parse",
            Self::Theme => "theme",
            Self::Runtime => "runtime",
            Self::Lint => "lint",
            Self::Geometry => "geometry",
        }
    }

    pub(crate) fn sort_rank(self) -> u8 {
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
pub(crate) struct CssCheckDiagnostic {
    // Severity decides both ordering and final result color
    pub(crate) severity: CssCheckSeverity,
    // Category keeps related warnings grouped together
    pub(crate) category: CssCheckCategory,
    // Stable codes make docs and tests easier to pin down
    pub(crate) code: &'static str,
    // This is already formatted for terminal display
    pub(crate) display_path: String,
    // Parser warnings may carry source locations
    pub(crate) line: Option<usize>,
    pub(crate) column: Option<usize>,
    pub(crate) message: String,
    // Hints stay optional so short warnings do not get bloated
    pub(crate) hint: Option<String>,
}

impl CssCheckDiagnostic {
    pub(crate) fn warning(
        category: CssCheckCategory,
        code: &'static str,
        display_path: String,
        message: impl Into<String>,
    ) -> Self {
        // Warnings do not carry parser locations
        Self {
            severity: CssCheckSeverity::Warning,
            category,
            code,
            display_path,
            line: None,
            column: None,
            message: message.into(),
            hint: None,
        }
    }

    pub(crate) fn error(
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
pub(crate) struct CssCheckActiveFile {
    // The slot name shows which config key resolved to this file
    pub(crate) slot_name: &'static str,
    pub(crate) display_path: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct CssCheckReport {
    // The displayed root may be $HOME or $XDG_CONFIG_HOME based
    pub(crate) display_root: String,
    pub(crate) checked_files: usize,
    // Active theme files are shown when the report needs more context
    pub(crate) active_files: Vec<CssCheckActiveFile>,
    // Notes are extra context that should not count as warnings
    pub(crate) notes: Vec<String>,
    pub(crate) diagnostics: Vec<CssCheckDiagnostic>,
}

impl CssCheckReport {
    pub(crate) fn error_count(&self) -> usize {
        // Errors decide whether css-check returns a failure
        self.diagnostics
            .iter()
            .filter(|item| item.severity == CssCheckSeverity::Error)
            .count()
    }

    pub(crate) fn warning_count(&self) -> usize {
        // Warnings keep the run successful but change the final verdict
        self.diagnostics
            .iter()
            .filter(|item| item.severity == CssCheckSeverity::Warning)
            .count()
    }

    pub(crate) fn is_clean(&self) -> bool {
        // A clean report means no diagnostics and no extra notes
        self.diagnostics.is_empty() && self.notes.is_empty()
    }
}
