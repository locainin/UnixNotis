//! Service-manager backend contract for installer-owned daemon startup.

mod artifact;
mod command;
mod dinit;
mod manager;
mod runit;
mod systemd;

pub use artifact::{ServiceArtifact, ServiceArtifactKind};
pub use command::CommandSpec;
pub use manager::ServiceManager;
#[cfg(test)]
pub use manager::{
    UNIXNOTIS_DAEMON_DINIT_SERVICE, UNIXNOTIS_DAEMON_RUNIT_SERVICE, UNIXNOTIS_DAEMON_SERVICE,
};

#[cfg(test)]
#[path = "service_manager/tests/artifacts.rs"]
mod artifact_tests;

#[cfg(test)]
#[path = "service_manager/tests/systemd.rs"]
mod systemd_tests;

#[cfg(test)]
#[path = "service_manager/tests/dinit.rs"]
mod dinit_tests;

#[cfg(test)]
#[path = "service_manager/tests/runit.rs"]
mod runit_tests;
