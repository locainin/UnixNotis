//! Bounded inline reply editor for verified communication notifications

mod lifecycle;
mod widget;

pub(in crate::ui::entry) use widget::build_inline_reply;

#[cfg(test)]
mod tests;
