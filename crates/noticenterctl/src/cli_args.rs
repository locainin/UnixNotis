//! CLI argument and command definitions for noticenterctl.

use clap::{Parser, Subcommand, ValueEnum};
use unixnotis_core::{PanelDebugLevel, INHIBIT_SCOPE_ALL, INHIBIT_SCOPE_POPUPS};

#[derive(Parser, Debug)]
#[command(author, version, about)]
pub(crate) struct Args {
    // Subcommands map 1:1 to the daemon control surface.
    #[command(subcommand)]
    pub(crate) command: Command,
}

#[derive(Subcommand, Debug)]
pub(crate) enum Command {
    // Toggle the panel visibility without changing other state.
    TogglePanel,
    // Open the panel, optionally enabling debug logging for live diagnostics.
    OpenPanel {
        #[arg(long, value_enum, num_args = 0..=1, default_missing_value = "info")]
        debug: Option<DebugLevelArg>,
    },
    // Close the panel if it is visible.
    ClosePanel,
    // Set or toggle Do Not Disturb mode.
    Dnd {
        #[arg(value_enum)]
        state: DndState,
    },
    // Clear active notifications.
    Clear,
    // Dismiss a single notification by identifier.
    Dismiss {
        id: u32,
    },
    // List active notifications; full output requires diagnostic mode.
    ListActive {
        #[arg(long)]
        full: bool,
    },
    // List notification history; full output requires diagnostic mode.
    ListHistory {
        #[arg(long)]
        full: bool,
    },
    // Create a new inhibitor token.
    Inhibit {
        reason: String,
        #[arg(long, value_enum, default_value = "all")]
        scope: InhibitScopeArg,
    },
    // Remove an inhibitor by token.
    Uninhibit {
        id: u64,
    },
    // Print current inhibitors to stdout.
    ListInhibitors,
    // Validate theme CSS files without touching D-Bus.
    CssCheck,
}

impl Command {
    pub(crate) fn is_css_check(&self) -> bool {
        // Helper keeps main logic concise and easy to scan.
        matches!(self, Command::CssCheck)
    }
}

#[derive(ValueEnum, Debug, Clone, Copy)]
pub(crate) enum DndState {
    // Explicitly enable DND.
    On,
    // Explicitly disable DND.
    Off,
    // Toggle based on current daemon state.
    Toggle,
}

#[derive(ValueEnum, Debug, Clone, Copy)]
pub(crate) enum DebugLevelArg {
    // Only critical diagnostic output.
    Critical,
    // Warnings and above.
    Warn,
    // Informational output (default).
    Info,
    // Verbose diagnostics.
    Verbose,
}

#[derive(ValueEnum, Debug, Clone, Copy)]
pub(crate) enum InhibitScopeArg {
    // Suppress both panel and popup updates.
    All,
    // Suppress popup updates only.
    Popups,
}

impl InhibitScopeArg {
    pub(crate) fn as_scope(self) -> u32 {
        // Map CLI scope to the daemon bitmask value.
        match self {
            Self::All => INHIBIT_SCOPE_ALL,
            Self::Popups => INHIBIT_SCOPE_POPUPS,
        }
    }
}

impl From<DebugLevelArg> for PanelDebugLevel {
    fn from(value: DebugLevelArg) -> Self {
        match value {
            DebugLevelArg::Critical => PanelDebugLevel::Critical,
            DebugLevelArg::Warn => PanelDebugLevel::Warn,
            DebugLevelArg::Info => PanelDebugLevel::Info,
            DebugLevelArg::Verbose => PanelDebugLevel::Verbose,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn parses_open_panel_debug_default() {
        // Ensures clap default_missing_value maps --debug to the Info level.
        let args =
            Args::try_parse_from(["noticenterctl", "open-panel", "--debug"]).expect("parse args");
        match args.command {
            Command::OpenPanel { debug } => {
                assert!(matches!(debug, Some(DebugLevelArg::Info)));
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parses_open_panel_debug_value() {
        // Verifies explicit debug values map to the requested verbosity.
        let args = Args::try_parse_from(["noticenterctl", "open-panel", "--debug", "verbose"])
            .expect("parse args");
        match args.command {
            Command::OpenPanel { debug } => {
                assert!(matches!(debug, Some(DebugLevelArg::Verbose)));
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parses_dnd_toggle() {
        // Confirms the value enum accepts the toggle state for DND commands.
        let args = Args::try_parse_from(["noticenterctl", "dnd", "toggle"]).expect("parse args");
        match args.command {
            Command::Dnd { state } => {
                assert!(matches!(state, DndState::Toggle));
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn debug_level_arg_into_panel_level() {
        // Validates CLI debug levels map to the matching control plane enum.
        let table = [
            (DebugLevelArg::Critical, PanelDebugLevel::Critical),
            (DebugLevelArg::Warn, PanelDebugLevel::Warn),
            (DebugLevelArg::Info, PanelDebugLevel::Info),
            (DebugLevelArg::Verbose, PanelDebugLevel::Verbose),
        ];
        for (arg, expected) in table {
            let mapped: PanelDebugLevel = arg.into();
            assert_eq!(mapped, expected);
        }
    }

    #[test]
    fn parses_inhibit_default_scope() {
        // Ensures inhibit defaults to the "all" scope when omitted.
        let args = Args::try_parse_from(["noticenterctl", "inhibit", "focus"]).expect("parse args");
        match args.command {
            Command::Inhibit { scope, .. } => {
                assert!(matches!(scope, InhibitScopeArg::All));
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parses_inhibit_popups_scope() {
        // Confirms popups scope is accepted for inhibit calls.
        let args = Args::try_parse_from(["noticenterctl", "inhibit", "focus", "--scope", "popups"])
            .expect("parse args");
        match args.command {
            Command::Inhibit { scope, .. } => {
                assert!(matches!(scope, InhibitScopeArg::Popups));
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }
}
