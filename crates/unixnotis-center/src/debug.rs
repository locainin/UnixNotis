//! Debug logging helpers gated by panel debug level.

use std::sync::atomic::{AtomicU8, Ordering};

use tracing::{error, info, warn};
use unixnotis_core::PanelDebugLevel;

static DEBUG_LEVEL: AtomicU8 = AtomicU8::new(PanelDebugLevel::Off as u8);

pub fn set_level(level: PanelDebugLevel) {
    DEBUG_LEVEL.store(level as u8, Ordering::Relaxed);
}

pub fn level() -> PanelDebugLevel {
    match DEBUG_LEVEL.load(Ordering::Relaxed) {
        1 => PanelDebugLevel::Critical,
        2 => PanelDebugLevel::Warn,
        3 => PanelDebugLevel::Info,
        4 => PanelDebugLevel::Verbose,
        _ => PanelDebugLevel::Off,
    }
}

pub fn allows(level: PanelDebugLevel) -> bool {
    let current = self::level();
    current != PanelDebugLevel::Off && current >= level
}

pub fn log(level: PanelDebugLevel, message: impl FnOnce() -> String) {
    if !allows(level) {
        return;
    }
    let message = message();
    match level {
        PanelDebugLevel::Critical => {
            error!(debug_level = ?level, message = %message, "unixnotis debug");
        }
        PanelDebugLevel::Warn => {
            warn!(debug_level = ?level, message = %message, "unixnotis debug");
        }
        PanelDebugLevel::Info | PanelDebugLevel::Verbose | PanelDebugLevel::Off => {
            info!(debug_level = ?level, message = %message, "unixnotis debug");
        }
    }
}
