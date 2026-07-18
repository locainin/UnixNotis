//! Backend-specific service artifacts and lifecycle commands

pub(super) mod dinit;
pub(super) mod runit;
pub(super) mod s6;
pub(super) mod systemd;

#[cfg(test)]
mod tests;
