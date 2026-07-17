//! Hyprland-specific install helpers

mod block;
mod detect;
mod manage;
mod paths;
mod write_target;

// File reads and writes stay in the flow module so the root stays as a router
pub(in crate::actions) use manage::{ensure_hyprland_autostart, remove_hyprland_autostart};

#[cfg(test)]
mod tests;
