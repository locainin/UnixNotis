//! Popup row construction and bounded label handling

mod activation;
mod build;
mod builders;
mod commands;
mod presentation;
mod visibility;

pub(in crate::ui) use build::PopupEntry;
pub(in crate::ui) use commands::try_send_command;
pub(in crate::ui) use visibility::PopupVisibilityBinding;
