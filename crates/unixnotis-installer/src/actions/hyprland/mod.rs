//! Hyprland-specific install helpers

mod block;
mod detect;
mod manage;
mod paths;

// File reads and writes stay in the flow module so the root stays as a router
pub(in crate::actions) use manage::{ensure_hyprland_autostart, remove_hyprland_autostart};

#[cfg(test)]
pub(in crate::actions::hyprland) use block::{
    strip_hyprland_bootstrap_block, HYPR_BOOTSTRAP_END, HYPR_BOOTSTRAP_START,
};

#[cfg(test)]
mod tests;
