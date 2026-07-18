//! CSS validation and lint helpers for `UnixNotis` themes

mod builder;
mod cache;
mod command;
mod files;
mod geometry;
mod lint;
mod parse;
mod policy;
mod report;
mod runtime;
mod theme;

pub use self::builder::build_report;
pub use self::command::run;

#[cfg(test)]
#[path = "tests/cases.rs"]
mod tests;
