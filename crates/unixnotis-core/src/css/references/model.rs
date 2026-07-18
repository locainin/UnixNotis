//! CSS reference records returned by the shared scanner

use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CssUrlSpan {
    // Raw payload stays unchanged so callers can apply their own URI policy
    pub value: String,
    // Byte indexes point into the original UTF-8 stylesheet for exact rewrites
    pub value_start: usize,
    pub value_end: usize,
    // Payload escapes need full value decoding and therefore remain fail-closed
    pub ambiguous: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CssReference {
    pub value: String,
    pub ambiguous: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CssImportReference {
    Target(String),
    Ambiguous,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum CssReferenceError {
    #[error("CSS contains an unterminated url(...) reference")]
    UnterminatedUrl,
    #[error("CSS reference scanning stopped because a token did not advance")]
    ScannerDidNotAdvance,
    #[error("CSS file contains more than {0} URL references")]
    TooManyUrls(usize),
    #[error("CSS file contains more than {0} import references")]
    TooManyImports(usize),
}
