//! Reusable child clipping for true diagonal card corners

mod geometry;
mod widget;

#[cfg(test)]
#[path = "tests/mod.rs"]
mod tests;

pub use widget::CutCorner;
