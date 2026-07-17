use std::path::PathBuf;

use super::*;

#[test]
fn explicit_source_classification_covers_cli_and_environment_only() {
    assert!(ConfigPathSource::Cli.is_explicit());
    assert!(ConfigPathSource::Environment.is_explicit());
    assert!(!ConfigPathSource::Default.is_explicit());
    assert!(!ConfigPathSource::Builtin.is_explicit());
}

#[test]
fn cli_path_is_returned_without_default_path_discovery() {
    let path = PathBuf::from("relative/config.toml");

    let resolved = resolve_config_path(Some(path.clone())).expect("resolve CLI path");

    assert_eq!(resolved, (path, ConfigPathSource::Cli));
}
