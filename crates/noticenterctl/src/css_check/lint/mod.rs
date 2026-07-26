//! CSS declaration, selector, and compatibility lint rules

mod directives;
mod runner;
mod scan;
mod values;

pub(in crate::css_check) use runner::{lint_css_files, CssCheckLintFinding};

#[cfg(test)]
#[path = "tests/support.rs"]
pub(in crate::css_check) mod test_support;
