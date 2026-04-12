//! Local-only preset command entry point
//!
//! Keeps preset export, import, inspect, and archive helpers in one module tree
//! so the local-only share flow stays separate from the D-Bus control path

mod archive;
pub(crate) mod command_rules;
mod config_root;
pub(crate) mod css_asset_refs;
mod export;
mod filesystem;
mod import;
mod inspect;
mod manifest;
mod pathing;

use anyhow::Result;
use std::path::Path;

use crate::cli_args::PresetCommand;

pub(crate) fn run_preset(command: PresetCommand) -> Result<()> {
    // Preset commands stay local so sharing configs does not depend on a running daemon
    match command {
        PresetCommand::Export {
            output,
            except,
            force,
        } => export::run_export(Path::new(&output), &except, force),
        PresetCommand::Import {
            input,
            except,
            dry_run,
        } => import::run_import(Path::new(&input), &except, dry_run),
        PresetCommand::Inspect { input } => inspect::run_inspect(Path::new(&input)),
    }
}
