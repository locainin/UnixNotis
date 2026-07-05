use clap::{Parser, Subcommand, ValueEnum};
use unixnotis_core::{PanelDebugLevel, INHIBIT_SCOPE_ALL, INHIBIT_SCOPE_POPUPS};

use super::route::Command;

#[derive(Parser, Debug)]
#[command(author, version, about)]
pub(crate) struct Args {
    // Subcommands map 1:1 to the daemon control surface
    #[command(subcommand)]
    pub(crate) command: Command,
}

#[derive(ValueEnum, Debug, Clone, Copy)]
pub(crate) enum DebugLevelArg {
    // Only critical diagnostic output
    Critical,
    // Warnings and above
    Warn,
    // Informational output
    Info,
    // Verbose diagnostics
    Verbose,
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

#[derive(ValueEnum, Debug, Clone, Copy)]
pub(crate) enum DndState {
    // Explicitly enable DND
    On,
    // Explicitly disable DND
    Off,
    // Toggle based on current daemon state
    Toggle,
}

#[derive(ValueEnum, Debug, Clone, Copy)]
pub(crate) enum InhibitScopeArg {
    // Suppress both panel and popup updates
    All,
    // Suppress popup updates only
    Popups,
}

impl InhibitScopeArg {
    pub(crate) fn as_scope(self) -> u32 {
        // Map CLI scope to the daemon bitmask value
        match self {
            Self::All => INHIBIT_SCOPE_ALL,
            Self::Popups => INHIBIT_SCOPE_POPUPS,
        }
    }
}

#[derive(Subcommand, Debug)]
pub(crate) enum PresetCommand {
    // Export the current config tree into one shareable bundle file
    Export {
        output: String,
        #[arg(long = "except", value_name = "PATH")]
        except: Vec<String>,
        #[arg(long)]
        force: bool,
    },
    // Import a bundle into the current config tree
    Import {
        input: String,
        #[arg(long = "except", value_name = "PATH")]
        except: Vec<String>,
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        allow_exec: bool,
    },
    // Print bundle metadata and included files without writing anything
    Inspect {
        input: String,
    },
}
