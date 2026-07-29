//! Explicit stock theme migration UI flow

mod actions;
mod flow;

pub(in crate::ui) use actions::connect_notice_actions;

#[cfg(test)]
mod tests;
