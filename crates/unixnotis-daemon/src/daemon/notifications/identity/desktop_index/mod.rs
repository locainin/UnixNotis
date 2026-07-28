//! Desktop application index preserving system and user entry origins

mod index;
mod launch;
pub(in crate::daemon::notifications::identity) mod model;
mod names;
mod program;
mod record;
mod refresh;
mod scan;
mod verification;
mod wrappers;

pub use model::DesktopIdentityIndex;
pub(super) use model::DesktopRecord;
pub(super) use model::{LaunchFailure, LaunchVerification};
pub(super) use names::{normalize_desktop_id, normalize_name};
pub use refresh::spawn_desktop_index_refresh;
pub use scan::DesktopIndexSnapshot;

pub(in crate::daemon::notifications::identity) fn verify_record_launch(
    record: &DesktopRecord,
    index: &DesktopIdentityIndex,
    sender_identity: super::FileIdentity,
    cmdline: &super::sender::CommandLineEvidence,
) -> LaunchVerification {
    verification::verify_record_launch(record, index, sender_identity, cmdline)
}

#[cfg(test)]
mod tests;
