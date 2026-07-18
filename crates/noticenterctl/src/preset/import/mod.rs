//! Preset import validation, review, and transactional application

pub(in crate::preset) mod command;
mod css_assets;
pub(in crate::preset) mod review;
pub(in crate::preset) mod transaction;

pub(in crate::preset) use command::run_import;

#[cfg(test)]
mod tests;
