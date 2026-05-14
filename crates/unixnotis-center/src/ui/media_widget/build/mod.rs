//! Media shell assembly and structural layout helpers
//!
//! Splits planning, shell composition, and widget assembly into focused files

mod plan;
mod shell;
mod widgets;

#[cfg(test)]
#[path = "tests/build.rs"]
mod tests;

// Widget assembly is the public entry for the surrounding media widget module
pub(super) use self::widgets::build_media_widget;
