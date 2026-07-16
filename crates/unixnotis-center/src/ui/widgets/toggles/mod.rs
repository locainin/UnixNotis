//! Toggle grid rendering, kind styling, icons, and state synchronization

mod css;
mod grid;
mod icons;
mod state;

pub use grid::ToggleGrid;

#[cfg(test)]
use grid::{should_reset_after_action, toggle_action_command};
#[cfg(test)]
#[path = "tests/grid.rs"]
mod tests;
