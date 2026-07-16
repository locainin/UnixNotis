//! CSS declaration, selector, and compatibility lint rules

mod runner;
mod scan;
mod values;

#[cfg(test)]
pub(in crate::css_check) use runner::lint_css_contents;
pub(in crate::css_check) use runner::{lint_css_files, CssCheckLintFinding};
