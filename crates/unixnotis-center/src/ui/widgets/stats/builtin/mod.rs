//! Built-in statistic sources and refresh infrastructure

mod detect;
mod model;
mod readers;

pub(super) use model::{BuiltinStat, BuiltinStatKey};

#[cfg(test)]
#[path = "../tests/builtin.rs"]
mod tests;
