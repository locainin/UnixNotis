use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

use super::super::{
    ensure_path_entry_in_file, format_path_for_shell_line, shell_path_entry_exists,
    shell_startup_files,
};

#[test]
fn shell_startup_files_prefers_zsh_and_profile() {
    let home = std::path::PathBuf::from("/tmp/unixnotis-home");
    let files = shell_startup_files(&home, Some("/usr/bin/zsh"));
    assert_eq!(files, vec![home.join(".zshrc"), home.join(".profile")]);
}

#[test]
fn ensure_path_entry_in_file_is_idempotent() {
    let root = test_root("path-entry-idempotent");
    let home = root.join("home");
    let bin_dir = home.join(".local").join("bin");
    let startup = home.join(".zshrc");

    fs::create_dir_all(&home).expect("create home");
    let first = ensure_path_entry_in_file(&startup, &home, &bin_dir).expect("first write");
    let second = ensure_path_entry_in_file(&startup, &home, &bin_dir).expect("second write");
    let contents = fs::read_to_string(&startup).expect("read startup");
    assert!(first);
    assert!(!second);
    assert!(shell_path_entry_exists(&contents, &home, &bin_dir));

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn format_path_for_shell_line_uses_home_prefix_when_possible() {
    let home = std::path::PathBuf::from("/tmp/unixnotis-home");
    let bin_dir = home.join(".local").join("bin");
    assert_eq!(
        format_path_for_shell_line(&home, &bin_dir),
        "$HOME/.local/bin"
    );
}

fn test_root(name: &str) -> std::path::PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "unixnotis-installer-env-{name}-{}-{stamp}",
        std::process::id()
    ))
}
