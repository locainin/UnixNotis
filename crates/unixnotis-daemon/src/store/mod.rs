//! Notification store with ordering, history, and suppression policies

mod dnd;
mod inhibitors;
mod model;
mod notifications;
mod runtime;

pub use model::{
    DismissOutcome, DndWrite, ExpirationTicket, InsertOutcome, NotificationStore, PopupAdmission,
    PopupSuppressionReason,
};

#[cfg(test)]
mod test_support;
#[cfg(test)]
mod tests;
