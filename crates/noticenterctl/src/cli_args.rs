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
    // Export, inspect, or import a shareable preset bundle.
    Preset {
        #[command(subcommand)]
        command: PresetCommand,
    },
}

impl Command {
    pub(crate) fn is_local_only(&self) -> bool {
        // Local-only commands should not fail just because D-Bus is unavailable.
        matches!(self, Command::CssCheck | Command::Preset { .. })
    }
}

#[derive(Subcommand, Debug)]
pub(crate) enum PresetCommand {
    // Export the current config tree into one shareable bundle file.
    Export {
        output: String,
        #[arg(long = "except", value_name = "PATH")]
        except: Vec<String>,
        #[arg(long)]
        force: bool,
    },
    // Import a bundle into the current config tree.
    Import {
        input: String,
        #[arg(long = "except", value_name = "PATH")]
        except: Vec<String>,
        #[arg(long)]
        dry_run: bool,
    },
    // Print bundle metadata and included files without writing anything.
    Inspect {
        input: String,
    },
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

    #[test]
    fn parses_preset_export_with_repeated_except() {
        // Repeated --except flags should preserve order for later filtering.
        let args = Args::try_parse_from([
            "noticenterctl",
            "preset",
            "export",
            "anime.unixnotis",
            "--except",
            "installer.toml",
            "--except",
            "assets/bg.png",
        ])
        .expect("parse args");
        match args.command {
            Command::Preset {
                command:
                    PresetCommand::Export {
                        output,
                        except,
                        force,
                    },
            } => {
                assert_eq!(output, "anime.unixnotis");
                assert_eq!(except, vec!["installer.toml", "assets/bg.png"]);
                assert!(!force);
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parses_preset_import_dry_run() {
        // Dry-run import should parse without touching D-Bus.
        let args = Args::try_parse_from([
            "noticenterctl",
            "preset",
            "import",
            "anime.unixnotis",
            "--dry-run",
        ])
        .expect("parse args");
        match args.command {
            Command::Preset {
                command:
                    PresetCommand::Import {
                        input,
                        except,
                        dry_run,
                    },
            } => {
                assert_eq!(input, "anime.unixnotis");
                assert!(except.is_empty());
                assert!(dry_run);
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn preset_commands_are_local_only() {
        // Preset commands should bypass D-Bus setup like css-check does.
        let args = Args::try_parse_from(["noticenterctl", "preset", "inspect", "anime.unixnotis"])
            .expect("parse args");
        assert!(args.command.is_local_only());
    }
}
