//! Configuration, tracing, and display readiness for daemon startup

mod config;
mod tracing;
mod wayland;

pub use config::load_config;
pub use tracing::init_tracing;
pub use wayland::ensure_wayland_session;

#[cfg(test)]
mod tests;
