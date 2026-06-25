//! Backend readiness findings used before service writes and checks

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ReadinessIssue {
    // The backend can continue, but the setup may not behave exactly as expected
    Warning(String),
    // The backend is missing a required tool or layout and should block install
    Error(String),
}

impl ReadinessIssue {
    pub fn warning(message: impl Into<String>) -> Self {
        // Use a constructor so callers do not repeat enum variant plumbing
        Self::Warning(message.into())
    }

    pub fn error(message: impl Into<String>) -> Self {
        // Errors are reserved for issues that would fail after writing artifacts
        Self::Error(message.into())
    }

    pub fn message(&self) -> &str {
        match self {
            // Both variants carry plain display text for logs and health checks
            Self::Warning(message) | Self::Error(message) => message,
        }
    }

    pub fn is_error(&self) -> bool {
        // Check rendering needs a cheap severity split without cloning strings
        matches!(self, Self::Error(_))
    }
}
