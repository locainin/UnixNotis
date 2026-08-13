//! Internal signals that drive bounded media refresh work

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::media) enum MediaRefreshOrigin {
    // Native bus traffic can justify one bounded fallback sweep
    Bus,
    // Synthetic retries never re-arm themselves because that would become polling
    Fallback,
}

#[derive(Debug)]
pub(in crate::media) enum MediaSignal {
    PropertiesChanged {
        bus_name: String,
        origin: MediaRefreshOrigin,
    },
    FairnessLeaseExpired {
        generation: u64,
    },
}
