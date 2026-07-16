//! Independent configuration, theme, bus, service, and log diagnostics

mod checks;
mod logs;
mod report;
mod service;

pub use report::run;
