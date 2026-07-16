//! Configuration module wiring for `UnixNotis`
//!
//! Keeps config types, I/O, and runtime cleanup in separate files

mod command_parse;
mod commands;
mod diagnostics;
mod icon_assets;
mod io;
mod layout;
mod media;
mod panel;
mod rules;
mod runtime;
mod schema;
mod theme;
mod types;
mod widgets;

pub use command_parse::{parse_command, CommandParseError, ExecutionMode, ParsedCommand};
pub use diagnostics::{
    log_config_diagnostics, ConfigDiagnostic, ConfigDiagnosticKind, ConfigLoadReport,
};
pub use icon_assets::{
    resolve_icon_asset_path, resolve_icon_asset_path_with_policy, validate_icon_asset_contents,
    validate_icon_asset_reference, AssetPolicy, IconAssetError, IconAssetResolver,
    ResolvedIconAsset, DEFAULT_ICON_ASSET_EXTENSIONS, DEFAULT_ICON_ASSET_MAX_BYTES,
    DEFAULT_ICON_ASSET_MAX_HEIGHT, DEFAULT_ICON_ASSET_MAX_PIXELS, DEFAULT_ICON_ASSET_MAX_WIDTH,
};
pub use io::{ConfigError, ThemePaths};
pub use layout::*;
pub use media::*;
pub use panel::*;
pub use rules::*;
pub use runtime::{MAX_CARD_WIDGETS, MAX_STAT_WIDGETS, MAX_TOGGLE_WIDGETS, MAX_TOTAL_WIDGETS};
pub use theme::*;
pub use types::*;
pub use widgets::*;
