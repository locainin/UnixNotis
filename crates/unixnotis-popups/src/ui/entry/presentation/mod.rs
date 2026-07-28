//! Popup-only presentation model derived from daemon-owned notification evidence

mod kind;
mod trust;
mod view_model;

pub(in crate::ui::entry) use kind::PopupKind;
pub(in crate::ui::entry) use trust::{PopupTrustPresentation, ReplyPresentation, TrustLevel};
pub(in crate::ui::entry) use view_model::{ActionViewModel, PopupEntryViewModel, ThumbnailKind};

#[cfg(test)]
#[path = "tests/mod.rs"]
mod tests;
