//! Configuration module wiring for `UnixNotis`
//!
//! Keeps config types, I/O, and runtime cleanup in separate files

mod appearance;
mod command;
mod installer_settings;
mod layout;
mod loading;
mod media;
mod panel;
mod reset;
mod runtime;
mod types;
mod validation;
mod widgets;

pub(in crate::config) use appearance::{corners, icon_assets, theme};
pub use corners::CutCorners;
pub use diagnostics::{
    log_config_diagnostics, ConfigDiagnostic, ConfigDiagnosticKind, ConfigLoadReport,
};
pub use icon_assets::{
    decode_image_asset_contents, materialize_bounded_image_as_png, resolve_icon_asset_path,
    resolve_icon_asset_path_with_policy, validate_icon_asset_contents,
    validate_icon_asset_reference, AssetPolicy, IconAssetError, IconAssetResolver,
    ResolvedIconAsset, DEFAULT_ICON_ASSET_EXTENSIONS, DEFAULT_ICON_ASSET_MAX_BYTES,
    DEFAULT_ICON_ASSET_MAX_HEIGHT, DEFAULT_ICON_ASSET_MAX_PIXELS, DEFAULT_ICON_ASSET_MAX_WIDTH,
};
pub use installer_settings::{
    ensure_installer_config, installer_config_path, load_installer_config, BackupConfig,
    InstallerConfig, DEFAULT_BACKUP_RETENTION, INSTALLER_CONFIG_FILE,
};
pub use io::{
    ConfigError, ThemeContractState, ThemeIncompatibility, ThemeManifest, ThemePaths,
    MAX_CONFIG_BYTES, THEME_API_VERSION,
};
pub use layout::*;
pub(in crate::config) use loading::{diagnostics, io};
pub use media::*;
pub use panel::*;
pub use reset::{
    render_default_config_toml, reset_config_to_defaults, ResetConfigOptions, ResetConfigReport,
};
pub use rules::*;
pub use runtime::{MAX_CARD_WIDGETS, MAX_STAT_WIDGETS, MAX_TOGGLE_WIDGETS, MAX_TOTAL_WIDGETS};
pub use theme::*;
pub use types::*;
pub(in crate::config) use validation::{rules, schema};
pub use widgets::*;
