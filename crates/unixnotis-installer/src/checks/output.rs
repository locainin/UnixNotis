//! Check item constructors

use super::{CheckItem, CheckState};

impl CheckItem {
    pub(super) fn ok(label: &'static str, detail: &str) -> Self {
        // Constructors keep row creation short in the check modules
        Self {
            label,
            state: CheckState::Ok,
            detail: detail.to_string(),
        }
    }

    pub(super) fn warn(label: &'static str, detail: &str) -> Self {
        Self {
            label,
            state: CheckState::Warn,
            detail: detail.to_string(),
        }
    }

    pub(super) fn fail(label: &'static str, detail: &str) -> Self {
        Self {
            label,
            state: CheckState::Fail,
            detail: detail.to_string(),
        }
    }
}
