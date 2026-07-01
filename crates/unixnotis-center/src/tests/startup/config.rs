use std::ffi::OsString;
use std::path::PathBuf;

use super::{config_override_path, Args};

#[test]
fn config_override_prefers_cli_arg() {
    let args = Args {
        config: Some(PathBuf::from("/tmp/cli.toml")),
    };

    // Direct CLI input wins over inherited daemon environment state
    assert_eq!(
        config_override_path(&args, Some(OsString::from("/tmp/env.toml"))),
        Some(PathBuf::from("/tmp/cli.toml"))
    );
}

#[test]
fn config_override_accepts_env_path() {
    let args = Args { config: None };

    // Daemon-spawned panels receive the exact config path through the env
    assert_eq!(
        config_override_path(&args, Some(OsString::from("/tmp/env.toml"))),
        Some(PathBuf::from("/tmp/env.toml"))
    );
}

#[test]
fn config_override_ignores_empty_env_path() {
    let args = Args { config: None };

    // Empty env values should behave like a missing override, not the cwd
    assert_eq!(config_override_path(&args, Some(OsString::new())), None);
}
