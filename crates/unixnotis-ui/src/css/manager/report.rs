//! Public CSS reload report model

use std::path::PathBuf;

use super::layers::CssProviderLayer;

/// Source used for one active CSS layer
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CssLayerSource {
    /// Embedded stock selected before any custom stylesheet was read
    EmbeddedStock,
    /// Non-empty configured file
    Custom,
    /// Embedded defaults selected by an intentionally empty file
    EmptyFallback,
    /// Embedded defaults selected after a file read failed
    ReadFailureFallback,
}

/// Result for one configured CSS provider layer
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CssLayerReload {
    /// Provider slot that was refreshed
    pub layer: CssProviderLayer,
    /// Configured path used for the read attempt
    pub path: PathBuf,
    /// Content source that reached GTK
    pub source: CssLayerSource,
    /// Sanitizable read failure detail when fallback followed an error
    pub error: Option<String>,
}

/// Complete result from refreshing the active CSS provider stack
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CssReloadReport {
    /// Ordered active provider results
    pub layers: Vec<CssLayerReload>,
}

impl CssReloadReport {
    /// Return layers that could not read their configured file
    pub fn read_failures(&self) -> impl Iterator<Item = &CssLayerReload> {
        self.layers
            .iter()
            .filter(|layer| layer.source == CssLayerSource::ReadFailureFallback)
    }
}

#[cfg(test)]
#[path = "tests/report.rs"]
mod tests;
