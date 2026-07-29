//! Popup row construction and bounded label handling

mod build;
mod builders;
mod commands;
mod presentation;

pub(in crate::ui) use build::PopupEntry;
pub(in crate::ui) use commands::try_send_command;
