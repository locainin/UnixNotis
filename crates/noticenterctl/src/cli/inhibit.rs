use clap::ValueEnum;
use unixnotis_core::{INHIBIT_SCOPE_ALL, INHIBIT_SCOPE_POPUPS};

#[derive(ValueEnum, Debug, Clone, Copy)]
pub(crate) enum InhibitScopeArg {
    // Suppress both panel and popup updates
    All,
    // Suppress popup updates only
    Popups,
}

impl InhibitScopeArg {
    pub(crate) fn as_scope(self) -> u32 {
        // Map CLI scope to the daemon bitmask value
        match self {
            Self::All => INHIBIT_SCOPE_ALL,
            Self::Popups => INHIBIT_SCOPE_POPUPS,
        }
    }
}
