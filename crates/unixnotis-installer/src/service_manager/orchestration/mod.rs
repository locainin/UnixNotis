//! Backend selection and unified installer-facing dispatch

mod artifacts;
mod environment;
mod lifecycle;
mod model;

pub use model::ServiceManager;

#[cfg(test)]
pub use model::{
    UNIXNOTIS_DAEMON_DINIT_SERVICE, UNIXNOTIS_DAEMON_RUNIT_SERVICE, UNIXNOTIS_DAEMON_S6_SERVICE,
    UNIXNOTIS_DAEMON_SERVICE,
};

#[cfg(test)]
mod tests;
