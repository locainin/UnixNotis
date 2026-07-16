//! Service diagnostic result types shared with log collection

use unixnotis_core::service_manager::ServiceManagerKind;

use super::super::report::DoctorCheck;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::doctor) enum SelectedServiceManager {
    // Managed variants carry a backend with installed status semantics
    Managed(ServiceManagerKind),
    // Manual means the control interface is live without an active manager
    Manual,
    // Unknown keeps insufficient evidence explicit rather than guessing
    Unknown,
}

pub(in crate::doctor) struct DoctorServiceResult {
    // Log routing consumes the same selection made by status diagnostics
    pub selected: SelectedServiceManager,
    // Every service check is retained even when an earlier probe fails
    pub checks: Vec<DoctorCheck>,
}
