//! Versioned release generation installation and recovery

mod entrypoints;
mod manifest;
mod transaction;

pub(in crate::actions) use manifest::{
    entrypoint_target, inspect_installed_generation, BinaryHealth,
};
pub use transaction::rollback_pending_release;
pub use transaction::{
    commit_pending_release, install_release_generation_transaction, pending_release_exists,
    pending_release_has_runtime_rollback,
};

#[cfg(test)]
mod tests;
