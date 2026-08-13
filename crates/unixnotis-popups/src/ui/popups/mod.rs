//! Popup collection reconciliation, mutation, and visibility

mod mutation;
mod reconcile;
mod timeout;
mod visibility;

pub(in crate::ui) use timeout::PopupHideTimer;
