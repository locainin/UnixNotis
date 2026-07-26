//! Desktop application index preserving system and user entry origins

mod index;
mod model;
mod names;
mod program;
mod record;
mod scan;

pub(in crate::daemon) use model::DesktopIdentityIndex;
pub(super) use model::DesktopRecord;
pub(super) use names::{normalize_desktop_id, normalize_name};

#[cfg(test)]
pub(super) use names::is_shared_launcher;
#[cfg(test)]
pub(super) use program::desktop_executable;
#[cfg(test)]
pub(super) use scan::{ScanBudget, ScanLimits};
#[cfg(test)]
mod tests;
