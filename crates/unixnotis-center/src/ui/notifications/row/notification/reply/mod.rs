//! Inline reply form wiring

mod binding;
mod build;
mod lifecycle;
mod presentation;
mod state;

pub(super) use binding::{configure_inline_reply, connect_inline_reply_button};
pub(super) use build::build_inline_reply;
pub(super) use state::InlineReplyWidgets;

#[cfg(test)]
mod tests;
