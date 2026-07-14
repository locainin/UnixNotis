//! Preset import flow for applying a bundle into the live config tree
//!
//! Import validates the bundle first, builds a write plan, optionally reports it,
//! then commits the final backup snapshot only after the staged import is ready to finish

mod apply;
mod checks;
mod commit;
mod exec_review;
mod plan;
mod prepare;
mod prompts;
mod runner;
mod summary;
#[cfg(test)]
#[path = "tests/helpers.rs"]
mod test_helpers;

pub(super) use self::runner::run_import;

#[cfg(test)]
mod tests;
