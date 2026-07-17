//! Internal events used to coalesce MPRIS refresh work

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaRefreshOrigin {
    // Native bus traffic can justify one bounded fallback sweep
    Bus,
    // Synthetic retries never re-arm themselves because that would become polling
    Fallback,
}

#[derive(Debug)]
pub enum MediaSignal {
    PropertiesChanged {
        bus_name: String,
        origin: MediaRefreshOrigin,
    },
}

#[cfg(test)]
#[path = "tests/signals.rs"]
mod tests;
