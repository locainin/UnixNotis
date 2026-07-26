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
