use clap::{Parser, Subcommand, ValueEnum};
use unixnotis_core::{PanelDebugLevel, INHIBIT_SCOPE_ALL, INHIBIT_SCOPE_POPUPS};

use super::command::Command;

#[derive(Parser, Debug)]
#[command(author, version, about)]
pub struct Args {
    // Subcommands map 1:1 to the daemon control surface
    #[command(subcommand)]
    pub(crate) command: Command,
}

#[derive(ValueEnum, Debug, Clone, Copy)]
pub enum DebugLevelArg {
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
            DebugLevelArg::Critical => Self::Critical,
            DebugLevelArg::Warn => Self::Warn,
            DebugLevelArg::Info => Self::Info,
            DebugLevelArg::Verbose => Self::Verbose,
        }
    }
}

#[derive(ValueEnum, Debug, Clone, Copy)]
pub enum DndState {
    // Explicitly enable DND
    On,
    // Explicitly disable DND
    Off,
    // Toggle based on current daemon state
    Toggle,
}

#[derive(ValueEnum, Debug, Clone, Copy)]
pub enum InhibitScopeArg {
    // Suppress both panel and popup updates
    All,
    // Suppress popup updates only
    Popups,
}

#[derive(ValueEnum, Debug, Clone, Copy, Eq, PartialEq)]
pub enum DoctorServiceManagerArg {
    // Inspect installed artifacts and active state without guessing between matches
    Auto,
    // Inspect the systemd user unit
    Systemd,
    // Inspect the dinit user service
    Dinit,
    // Inspect the runit user service
    Runit,
    // Inspect the s6-rc user service
    S6,
    // Treat the daemon as a manually launched process
    Manual,
}

impl InhibitScopeArg {
    pub(crate) const fn as_scope(self) -> u32 {
        // Map CLI scope to the daemon bitmask value
        match self {
            Self::All => INHIBIT_SCOPE_ALL,
            Self::Popups => INHIBIT_SCOPE_POPUPS,
        }
    }
}

#[derive(Subcommand, Debug)]
pub enum PresetCommand {
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
