use clap::Subcommand;

use super::args::{DebugLevelArg, DndState, InhibitScopeArg, PresetCommand};

#[derive(Subcommand, Debug)]
pub(crate) enum Command {
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
    CssCheck,
    // Export, inspect, or import a shareable preset bundle
    Preset {
        #[command(subcommand)]
        command: PresetCommand,
    },
}

impl Command {
    pub(crate) fn is_local_only(&self) -> bool {
        // Local-only commands should not fail just because D-Bus is unavailable
        matches!(self, Command::CssCheck | Command::Preset { .. })
    }
}
