//! Notification store with ordering, history, and suppression policies

mod dnd;
mod inhibitors;
mod model;
mod notifications;
mod runtime;

pub use model::{
    CloseAuthorization, CommitDisposition, DeliveryStageUpdate, DismissOutcome, DndWrite,
    ExpirationTicket, InsertOutcome, NotificationStore, PopupAdmission, PopupSuppressionReason,
    StableProcessIdentity, SuppressedNotification,
};

#[cfg(test)]
pub mod test_support;
#[cfg(test)]
mod tests;
