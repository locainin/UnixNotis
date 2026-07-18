//! Result types for one CSS file load

/// Source used for the CSS bytes passed to GTK
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::css) enum CssFileLoadSource {
    /// Non-empty custom CSS was read from disk
    Custom,
    /// An intentionally empty file used embedded defaults
    EmptyFallback,
    /// A read error caused embedded defaults to be used
    ReadFailureFallback,
}

/// Internal result from loading one configured stylesheet
#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::css) struct CssFileLoadResult {
    pub(in crate::css) source: CssFileLoadSource,
    pub(in crate::css) error: Option<String>,
}

impl CssFileLoadResult {
    pub(in crate::css) const fn custom() -> Self {
        Self {
            source: CssFileLoadSource::Custom,
            error: None,
        }
    }

    pub(in crate::css) const fn empty_fallback() -> Self {
        Self {
            source: CssFileLoadSource::EmptyFallback,
            error: None,
        }
    }

    pub(in crate::css) const fn read_failure(error: String) -> Self {
        Self {
            source: CssFileLoadSource::ReadFailureFallback,
            error: Some(error),
        }
    }
}

#[cfg(test)]
#[path = "../tests/loader/model.rs"]
mod tests;
