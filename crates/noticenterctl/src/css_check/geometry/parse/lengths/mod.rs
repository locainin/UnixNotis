//! CSS length parsing split by shorthand, expression, token, and function logic

mod edges;
mod expression;
mod resolve_calc;
mod resolve_compare;
mod resolve_var;
mod tokenize;
mod units;

use super::CssCustomProperties;

pub(in super::super) use edges::set_edge;
pub(in super::super) use edges::{parse_box_edges, parse_box_vertical_edges, parse_single_length};
pub(super) use expression::{parse_length_expression, ResolvedCssValue};

#[cfg(test)]
#[path = "tests/cases.rs"]
mod tests;
