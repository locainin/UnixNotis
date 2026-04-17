//! Media shell composition helpers
//!
//! Keeps shell assembly split by concern so future media presets can add or
//! replace one piece without reopening a monolith

mod alignment;
mod compose;
mod strips;

pub(super) use self::compose::compose_card_shell;
