//! Service-manager selection values

// Installer and diagnostics share one parser so backend aliases cannot drift
pub use unixnotis_core::service_manager::ServiceManagerKind as ServiceManagerChoice;
