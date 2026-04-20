//! Small CSS scanner helpers for css-check

mod blocks;
mod declarations;
mod selectors;
mod types;

pub(super) use blocks::{next_css_block, next_css_block_with_offsets, strip_css_comments};
pub(super) use declarations::{parse_css_declarations, parse_css_declarations_with_offsets};
pub(super) use selectors::{normalize_selector, should_recurse_at_rule, split_selectors};
