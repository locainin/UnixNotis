//! Backend selection and unified installer-facing dispatch

mod artifacts;
mod environment;
mod lifecycle;
mod model;

pub use model::ServiceManager;

#[cfg(test)]
mod tests;
