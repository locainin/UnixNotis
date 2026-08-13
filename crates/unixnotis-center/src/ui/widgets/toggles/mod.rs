//! Toggle grid rendering, kind styling, icons, and state synchronization

mod css;
mod grid;
mod icons;
mod rfkill;
mod state;

pub use grid::ToggleGrid;

#[cfg(test)]
#[path = "tests/grid.rs"]
mod tests;
