//! Command execution, refresh backoff, and persistent watch lifecycles

pub(in crate::ui::widgets) mod backoff;
pub(in crate::ui::widgets) mod command;
pub(in crate::ui::widgets) mod watch;
mod watch_reaper;
