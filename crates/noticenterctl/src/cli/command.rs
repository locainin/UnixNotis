use std::path::PathBuf;

use clap::Subcommand;

use super::args::{
    DevCommand, DndState, DoctorCommand, DoctorServiceManagerArg, PresetCommand, ThemeCommand,
};
use super::InhibitScopeArg;
use super::{DndClockTime, DndDuration};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExecutionKind {
    // Runs without Tokio or a daemon connection
    LocalSync,
    // Runs on Tokio but owns any D-Bus connections it needs
    LocalAsync,
    // Uses the shared daemon control bootstrap and API version check
    Daemon,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    // Toggle the panel visibility without changing other state
    TogglePanel,
    // Open the panel without enabling diagnostic rendering
    OpenPanel,
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
    // Clear active notifications without deleting saved history
    ClearActive,
    // Clear saved history without closing active notifications
    ClearHistory,
    // Dismiss a single notification by identifier
    Dismiss {
        id: u32,
    },
    // List active notifications using bounded terminal-safe output
    ListActive,
    // List notification history using bounded terminal-safe output
    ListHistory,
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
        #[command(subcommand)]
        command: Option<DoctorCommand>,
        #[arg(long)]
        json: bool,
        #[arg(long)]
        verbose: bool,
        #[arg(long, value_enum, default_value = "auto", global = true)]
        service_manager: DoctorServiceManagerArg,
        #[arg(long, value_name = "PATH")]
        config: Option<PathBuf>,
    },
    // Export, inspect, or import a shareable preset bundle
    Preset {
        #[command(subcommand)]
        command: PresetCommand,
    },
    // Export editable bundled theme files without changing the active configuration
    Theme {
        #[command(subcommand)]
        command: ThemeCommand,
    },
    // Keep maintenance commands discoverable only through explicit dev help
    #[command(hide = true)]
    Dev {
        #[command(subcommand)]
        command: DevCommand,
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

        if let Self::Doctor {
            command: Some(DoctorCommand::RepairSession),
            json,
            verbose,
            service_manager,
            config,
        } = self
        {
            // Report-only options must never be accepted and then ignored during repair
            if *json || *verbose || config.is_some() {
                return Err(anyhow::anyhow!(
                    "--json, --verbose, and --config are not valid with `doctor repair-session`"
                ));
            }

            // Manual mode has no installed service whose environment can be repaired
            if matches!(service_manager, DoctorServiceManagerArg::Manual) {
                return Err(anyhow::anyhow!(
                    "--service-manager manual is not valid with `doctor repair-session`"
                ));
            }
        }
        Ok(())
    }

    pub(crate) const fn execution_kind(&self) -> ExecutionKind {
        // One classification prevents contradictory local and synchronous flags
        match self {
            Self::CssCheck { .. }
            | Self::Preset { .. }
            | Self::Theme { .. }
            | Self::Doctor {
                command: Some(DoctorCommand::RepairSession),
                ..
            }
            | Self::Dev {
                command: DevCommand::Logs,
            } => ExecutionKind::LocalSync,
            Self::Doctor { command: None, .. } => ExecutionKind::LocalAsync,
            _ => ExecutionKind::Daemon,
        }
    }
}
