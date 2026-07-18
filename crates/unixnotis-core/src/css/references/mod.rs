//! Shared CSS URI reference tokenization
//!
//! Security checks and runtime rewriting must agree on decoded CSS token names

mod lexer;
mod model;
mod url;

pub use model::{CssReference, CssReferenceError, CssUrlSpan};
pub use url::{collect_css_url_spans, collect_css_url_values};
