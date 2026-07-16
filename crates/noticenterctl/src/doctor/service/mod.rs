//! Service-manager selection, artifacts, and bounded status diagnostics

mod artifacts;
mod inspect;
mod model;
mod probe;
mod selection;

pub(super) use inspect::inspect_service_manager;
pub(super) use model::SelectedServiceManager;

#[cfg(test)]
mod tests;
