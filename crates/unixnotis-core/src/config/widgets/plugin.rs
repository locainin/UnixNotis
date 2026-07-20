use serde::{Deserialize, Serialize};

use crate::CommandSpec;

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(default)]
pub struct WidgetPluginConfig {
    /// Versioned widget plugin contract
    pub api_version: u32,
    /// Plugin command executed by the widget worker
    pub command: CommandSpec,
    /// Maximum allowed command runtime before timeout (milliseconds)
    pub timeout_ms: u64,
    /// Maximum accepted stdout payload size before parse rejection
    pub max_output_bytes: usize,
}

impl WidgetPluginConfig {
    pub const API_VERSION_V1: u32 = 1;
    const DEFAULT_TIMEOUT_MS: u64 = 2_000;
    const DEFAULT_MAX_OUTPUT_BYTES: usize = 16 * 1024;
}

impl Default for WidgetPluginConfig {
    fn default() -> Self {
        Self {
            api_version: Self::API_VERSION_V1,
            command: CommandSpec::direct("", std::iter::empty::<&str>()),
            timeout_ms: Self::DEFAULT_TIMEOUT_MS,
            max_output_bytes: Self::DEFAULT_MAX_OUTPUT_BYTES,
        }
    }
}
