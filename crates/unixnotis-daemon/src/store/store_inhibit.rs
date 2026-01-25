//! Inhibitor bookkeeping and scope evaluation.
//!
//! Split out to keep the main store focused on notification lifecycle logic.

use unixnotis_core::INHIBIT_SCOPE_POPUPS;

#[derive(Debug, Clone)]
pub(super) struct Inhibitor {
    pub(super) id: u64,
    pub(super) owner: String,
    pub(super) reason: String,
    pub(super) scope: u32,
}

#[derive(Debug)]
pub(crate) struct InhibitorOwnerMismatch {
    expected_owner: String,
    actual_owner: String,
}

impl InhibitorOwnerMismatch {
    pub(super) fn new(expected_owner: String, actual_owner: String) -> Self {
        Self {
            expected_owner,
            actual_owner,
        }
    }

    pub(crate) fn message(&self) -> String {
        format!(
            "inhibitor owned by {}, not {}",
            self.expected_owner, self.actual_owner
        )
    }
}

pub(super) fn inhibits_popups(scope: u32) -> bool {
    // Scope 0 is the legacy "all output" value; bit 0 specifically targets popups.
    scope == 0 || scope & INHIBIT_SCOPE_POPUPS != 0
}
