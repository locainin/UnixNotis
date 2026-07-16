use std::path::PathBuf;

use clap::Parser;

use super::Args;

#[test]
fn popup_command_accepts_an_explicit_configuration_path() {
    let args = Args::try_parse_from([
        "unixnotis-popups",
        "--config",
        "/tmp/unixnotis-popup-config.toml",
    ])
    .expect("parse popup configuration path");

    assert_eq!(
        args.config,
        Some(PathBuf::from("/tmp/unixnotis-popup-config.toml"))
    );
}

#[test]
fn popup_command_leaves_configuration_unset_by_default() {
    let args = Args::try_parse_from(["unixnotis-popups"]).expect("parse popup defaults");

    assert!(args.config.is_none());
}
