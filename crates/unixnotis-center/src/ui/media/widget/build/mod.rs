//! Media shell assembly and structural layout helpers
//!
//! Splits planning, shell composition, and widget assembly into focused files

mod plan;
mod shell;
mod widgets;

#[cfg(test)]
#[path = "tests/plan.rs"]
mod plan_tests;
#[cfg(test)]
#[path = "tests/widgets.rs"]
mod widgets_tests;

// Widget assembly is the public entry for the surrounding media widget module
pub(super) use self::widgets::build_media_widget;
