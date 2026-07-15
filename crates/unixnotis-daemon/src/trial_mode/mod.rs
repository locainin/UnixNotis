//! Trial-mode ownership replacement and restoration

mod control;
mod owner;
mod prompt;
mod state;

pub use control::restore_previous;
pub use state::{prepare_trial, RestoreAction, TrialState};
pub use state::{DetectedDaemon, OwnerInfo, KNOWN_DAEMONS, TRIAL_COMMAND_TIMEOUT};
