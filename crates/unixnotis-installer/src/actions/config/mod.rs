//! Config file actions and related helpers

mod provision;
mod state;

// Keep backup logic in its own directory so reset/create flows stay focused
pub(crate) mod backup;

// File creation and reset live together because they share default template work
pub(crate) use provision::{ensure_config, reset_config};
// State cleanup stays separate so uninstall paths do not drag file-creation details along
pub(crate) use state::remove_state;

#[cfg(test)]
mod tests;
