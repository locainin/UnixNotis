use std::ffi::OsString;
use std::path::PathBuf;

use super::{config_override_path, Args};

#[test]
fn config_override_prefers_cli_arg() {
    let args = Args {
        config: Some(PathBuf::from("/tmp/cli.toml")),
    };
    assert_eq!(
        config_override_path(&args, Some(OsString::from("/tmp/env.toml"))),
        Some(PathBuf::from("/tmp/cli.toml"))
    );
}

#[test]
fn config_override_accepts_env_path() {
    let args = Args { config: None };
    assert_eq!(
        config_override_path(&args, Some(OsString::from("/tmp/env.toml"))),
        Some(PathBuf::from("/tmp/env.toml"))
    );
}

#[test]
fn config_override_ignores_empty_env_path() {
    let args = Args { config: None };
    assert_eq!(config_override_path(&args, Some(OsString::new())), None);
}
