//! Shared CSS URI reference tokenization
//!
//! Security checks and runtime rewriting must agree on decoded CSS token names

mod import;
mod lexer;
mod model;
mod url;

pub use import::{
    collect_css_import_dependency_values, collect_css_import_url_spans, collect_css_import_values,
};
pub use model::{CssImportReference, CssReference, CssReferenceError, CssUrlSpan};
pub use url::{collect_css_url_spans, collect_css_url_values};

#[cfg(test)]
mod tests;
