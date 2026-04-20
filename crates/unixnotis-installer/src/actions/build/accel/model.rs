//! Shared types for build acceleration detection and writes

#[derive(Clone, Debug)]
pub struct BuildAccelDetection {
    // Tool presence is tracked separately so the UI can explain which fast path is available
    pub sccache_installed: bool,
    pub mold_installed: bool,
    pub config_status: BuildAccelConfigStatus,
}

#[derive(Clone, Debug)]
pub enum BuildAccelConfigStatus {
    // No repo-local Cargo config exists yet
    Missing,
    // Installer owns the config and can safely refresh it
    Managed { wrapper_present: bool },
    // A user-managed config exists and must be left alone
    Unmanaged,
    // Repo config could not be read, so only the error is safe to surface
    ReadFailed(String),
}

#[derive(Clone, Debug)]
pub enum BuildAccelOutcome {
    // Neither accelerator exists, so there is nothing useful to write
    SkippedMissingTools,
    // A non-installer config exists, so the write path must stop
    SkippedExistingConfig,
    Written {
        relative_path: String,
        used_sccache: bool,
        used_mold: bool,
    },
    UpdatedExisting {
        relative_path: String,
        used_sccache: bool,
        used_mold: bool,
    },
    // Write failures are flattened into a string so callers can log them directly
    Failed(String),
}
