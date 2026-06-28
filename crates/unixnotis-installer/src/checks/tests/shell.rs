use std::path::Path;

use super::{path_check_item_from, path_entries_match};
use crate::checks::CheckState;

#[test]
fn path_entries_match_accepts_exact_paths() {
    // Exact string matches should return before any canonical filesystem work
    assert!(path_entries_match(
        Path::new("/tmp/unixnotis-bin"),
        Path::new("/tmp/unixnotis-bin")
    ));
}

#[test]
fn shell_path_warns_when_bin_is_on_path_but_command_was_uninstalled() {
    let item = path_check_item_from("$HOME/.local/bin", true, false);

    // PATH can be correct after uninstall, so command presence is checked separately
    assert_eq!(item.state, CheckState::Warn);
    assert!(item.detail.contains("noticenterctl is not installed there"));
}

#[test]
fn shell_path_warns_when_fresh_shell_has_no_path_and_no_command() {
    let item = path_check_item_from("$HOME/.local/bin", false, false);

    // Fresh installs should explain both missing PATH and missing command state
    assert_eq!(item.state, CheckState::Warn);
    assert!(item.detail.contains("missing $HOME/.local/bin"));
    assert!(item.detail.contains("noticenterctl is not installed there"));
}

#[test]
fn shell_path_is_ok_only_when_path_and_command_are_both_present() {
    let item = path_check_item_from("$HOME/.local/bin", true, true);

    // Direct command usage is ready only when shell lookup and managed install agree
    assert_eq!(item.state, CheckState::Ok);
    assert!(item.detail.contains("noticenterctl is installed there"));
}
