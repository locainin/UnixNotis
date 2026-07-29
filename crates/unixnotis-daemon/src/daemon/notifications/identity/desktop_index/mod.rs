//! Desktop application index preserving system and user entry origins

mod index;
mod launch;
mod launcher;
pub(in crate::daemon::notifications::identity) mod model;
mod names;
mod program;
pub(in crate::daemon::notifications::identity) mod provenance;
mod record;
mod refresh;
mod scan;
mod verification;
mod wrappers;

pub use model::DesktopIdentityIndex;
pub(super) use model::DesktopRecord;
pub(super) use model::{LaunchFailure, LaunchVerification, VerifiedLaunch};
pub(super) use names::{normalize_desktop_id, normalize_name};
pub(super) use provenance::InstallProvenance;
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
