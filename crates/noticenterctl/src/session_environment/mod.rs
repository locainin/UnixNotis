//! Session environment synchronization without generated shell transactions

mod backends;
mod manager;
mod process;
mod sync;
mod variables;

pub use sync::sync;

#[cfg(test)]
mod tests;
