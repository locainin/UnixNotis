//! Desktop application index preserving system and user entry origins

mod index;
mod launch;
pub(in crate::daemon::notifications::identity) mod model;
mod names;
mod program;
mod record;
mod refresh;
mod scan;

pub use model::DesktopIdentityIndex;
pub(super) use model::DesktopRecord;
pub(super) use names::{normalize_desktop_id, normalize_name};
pub use refresh::spawn_desktop_index_refresh;

pub(in crate::daemon::notifications::identity) fn record_launch_matches(
    record: &DesktopRecord,
    sender_identity: super::FileIdentity,
    cmdline: Option<&[Vec<u8>]>,
) -> bool {
    match &record.launch_spec {
        None => true,
        Some(spec) => cmdline.is_some_and(|cmdline| {
            launch::launch_spec_matches_sender(spec, sender_identity, cmdline)
        }),
    }
}

#[cfg(test)]
mod tests;
