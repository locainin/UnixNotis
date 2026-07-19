use std::path::PathBuf;

use clap::Subcommand;

use super::args::{DndState, DoctorServiceManagerArg, PresetCommand};
use super::{DebugLevelArg, InhibitScopeArg};
use super::{DndClockTime, DndDuration};

#[derive(Subcommand, Debug)]
pub enum Command {
    // Toggle the panel visibility without changing other state
    TogglePanel,
    // Open the panel, optionally enabling debug logging for live diagnostics
    OpenPanel {
        #[arg(long, value_enum, num_args = 0..=1, default_missing_value = "info")]
        debug: Option<DebugLevelArg>,
    },
    // Close the panel if it is visible
    ClosePanel,
    // Set or toggle Do Not Disturb mode
    Dnd {
        #[arg(value_enum)]
        state: DndState,
        #[arg(long = "for", value_name = "DURATION", conflicts_with = "until")]
        for_duration: Option<DndDuration>,
        #[arg(long, value_name = "HH:MM", conflicts_with = "for_duration")]
        until: Option<DndClockTime>,
    },
    // Clear active notifications and saved history
    Clear,
    // Clear active notifications and saved history
    ClearAll,
    // Clear active notifications without deleting saved history
    ClearActive,
    // Clear saved history without closing active notifications
    ClearHistory,
    // Dismiss a single notification by identifier
    Dismiss {
        id: u32,
    },
    // List active notifications; full output requires diagnostic mode
    ListActive {
        #[arg(long)]
        full: bool,
    },
    // List notification history; full output requires diagnostic mode
    ListHistory {
        #[arg(long)]
        full: bool,
    },
    // Create a new inhibitor token
    Inhibit {
        reason: String,
        #[arg(long, value_enum, default_value = "all")]
        scope: InhibitScopeArg,
    },
    // Remove an inhibitor by token
    Uninhibit {
        id: u64,
    },
    // Print current inhibitors to stdout
    ListInhibitors,
    // Validate theme CSS files without touching D-Bus
    CssCheck {
        #[arg(long, value_name = "PATH")]
        config: Option<PathBuf>,
    },
    // Collect independent configuration, theme, bus, service, and log diagnostics
    Doctor {
        #[arg(long)]
        json: bool,
        #[arg(long)]
        verbose: bool,
        #[arg(long, value_enum, default_value = "auto")]
        service_manager: DoctorServiceManagerArg,
        #[arg(long, value_name = "PATH")]
        config: Option<PathBuf>,
    },
    // Export, inspect, or import a shareable preset bundle
    Preset {
        #[command(subcommand)]
        command: PresetCommand,
    },
}

impl Command {
    pub(crate) fn validate(&self) -> anyhow::Result<()> {
        if let Self::Dnd {
            state,
            for_duration,
            until,
        } = self
        {
            let has_deadline = for_duration.is_some() || until.is_some();
            if has_deadline && !matches!(state, DndState::On) {
                return Err(anyhow::anyhow!(
                    "--for and --until are valid only with `dnd on`"
                ));
            }
        }
        Ok(())
    }

    pub(crate) const fn is_local_only(&self) -> bool {
        // Local-only commands should not fail just because D-Bus is unavailable
        matches!(
            self,
            Self::CssCheck { .. } | Self::Doctor { .. } | Self::Preset { .. }
        )
    }

    pub(crate) const fn is_synchronous(&self) -> bool {
        // Doctor uses local inputs but still needs asynchronous D-Bus and process timeouts
        matches!(self, Self::CssCheck { .. } | Self::Preset { .. })
    }
}
