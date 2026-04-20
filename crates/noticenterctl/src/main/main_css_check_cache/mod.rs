//! Cache-aware GTK parse stage for css-check

mod dependencies;
mod model;
mod parse;
mod session;
mod store;

pub(super) use session::validate_css_parse_files;

#[cfg(test)]
pub(super) use model::{CachedParseDiagnostic, CssParseReport, CssParseWorkItem};

#[cfg(test)]
pub(super) use session::{parse_diagnostic_for_test, validate_css_parse_files_with};

#[cfg(test)]
mod tests;
