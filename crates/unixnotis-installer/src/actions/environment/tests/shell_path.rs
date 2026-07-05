use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

use super::super::{
    ensure_path_entry_in_file, format_path_for_shell_line, remove_path_entry_from_file,
    shell_path_entry_exists, shell_startup_files,
};

#[test]
fn shell_startup_files_prefers_zsh_and_profile() {
    let home = std::path::PathBuf::from("/tmp/unixnotis-home");
    let files = shell_startup_files(&home, Some("/usr/bin/zsh"));
    assert_eq!(files, vec![home.join(".zshrc"), home.join(".profile")]);
}

#[test]
fn shell_startup_files_prefers_bash_and_profile() {
    let home = std::path::PathBuf::from("/tmp/unixnotis-home");
    let files = shell_startup_files(&home, Some("/bin/bash"));
    assert_eq!(files, vec![home.join(".bashrc"), home.join(".profile")]);
}

#[test]
fn shell_startup_files_uses_only_profile_for_unknown_shell() {
    let home = std::path::PathBuf::from("/tmp/unixnotis-home");
    let files = shell_startup_files(&home, Some("/bin/fish"));
    assert_eq!(files, vec![home.join(".profile")]);
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
fn ensure_path_entry_in_file_separates_existing_content_without_trailing_newline() {
    let root = test_root("path-entry-existing-no-newline");
    let home = root.join("home");
    let bin_dir = home.join(".local").join("bin");
    let startup = home.join(".profile");

    fs::create_dir_all(&home).expect("create home");
    fs::write(&startup, "existing line").expect("write startup");

    let changed = ensure_path_entry_in_file(&startup, &home, &bin_dir).expect("path write");
    let contents = fs::read_to_string(&startup).expect("read startup");

    assert!(changed);
    assert!(contents.starts_with("existing line\n# unixnotis-installer path entry\n"));

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn ensure_path_entry_in_file_starts_empty_file_with_managed_block() {
    let root = test_root("path-entry-empty-start");
    let home = root.join("home");
    let bin_dir = home.join(".local").join("bin");
    let startup = home.join(".profile");

    fs::create_dir_all(&home).expect("create home");

    let changed = ensure_path_entry_in_file(&startup, &home, &bin_dir).expect("path write");
    let contents = fs::read_to_string(&startup).expect("read startup");

    assert!(changed);
    assert_eq!(
        contents,
        "# unixnotis-installer path entry\nexport PATH=\"$HOME/.local/bin:$PATH\"\n"
    );

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn ensure_path_entry_in_file_uses_existing_trailing_newline_without_blank_line() {
    let root = test_root("path-entry-existing-newline");
    let home = root.join("home");
    let bin_dir = home.join(".local").join("bin");
    let startup = home.join(".profile");

    fs::create_dir_all(&home).expect("create home");
    fs::write(&startup, "existing line\n").expect("write startup");

    let changed = ensure_path_entry_in_file(&startup, &home, &bin_dir).expect("path write");
    let contents = fs::read_to_string(&startup).expect("read startup");

    assert!(changed);
    assert_eq!(
        contents,
        "existing line\n# unixnotis-installer path entry\nexport PATH=\"$HOME/.local/bin:$PATH\"\n"
    );

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn ensure_path_entry_in_file_reports_directory_read_errors() {
    let root = test_root("path-entry-directory-read-error");
    let home = root.join("home");
    let bin_dir = home.join(".local").join("bin");
    let startup = home.join(".profile");

    fs::create_dir_all(&startup).expect("startup directory");

    let err = ensure_path_entry_in_file(&startup, &home, &bin_dir)
        .expect_err("directory should not be treated as missing");

    assert!(err.to_string().contains("failed to read"));

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn ensure_path_entry_in_file_detects_existing_absolute_path_entry() {
    let root = test_root("path-entry-existing-absolute");
    let home = root.join("home");
    let bin_dir = home.join(".local").join("bin");
    let startup = home.join(".profile");

    fs::create_dir_all(&home).expect("create home");
    fs::write(
        &startup,
        format!("export PATH=\"{}:$PATH\"\n", bin_dir.display()),
    )
    .expect("write existing path");

    let changed = ensure_path_entry_in_file(&startup, &home, &bin_dir).expect("path check");

    // Existing absolute entries should not gain a duplicate managed block
    assert!(!changed);
    assert!(!fs::read_to_string(&startup)
        .expect("read startup")
        .contains("# unixnotis-installer path entry"));

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn remove_path_entry_from_file_removes_only_managed_block() {
    let root = test_root("path-entry-remove-managed");
    let home = root.join("home");
    let bin_dir = home.join(".local").join("bin");
    let startup = home.join(".profile");

    fs::create_dir_all(&home).expect("create home");
    ensure_path_entry_in_file(&startup, &home, &bin_dir).expect("managed block");
    let changed = remove_path_entry_from_file(&startup, &home, &bin_dir).expect("remove block");
    let contents = fs::read_to_string(&startup).expect("read startup");

    assert!(changed);
    assert!(!contents.contains("# unixnotis-installer path entry"));
    assert!(contents.is_empty());
    assert!(!shell_path_entry_exists(&contents, &home, &bin_dir));

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn remove_path_entry_from_file_preserves_surrounding_content_and_trailing_newline() {
    let root = test_root("path-entry-remove-preserve-content");
    let home = root.join("home");
    let bin_dir = home.join(".local").join("bin");
    let startup = home.join(".profile");

    fs::create_dir_all(&home).expect("create home");
    fs::write(
        &startup,
        "before\n# unixnotis-installer path entry\nexport PATH=\"$HOME/.local/bin:$PATH\"\nafter\n",
    )
    .expect("startup contents");

    let changed = remove_path_entry_from_file(&startup, &home, &bin_dir).expect("remove block");
    let contents = fs::read_to_string(&startup).expect("read startup");

    assert!(changed);
    assert_eq!(contents, "before\nafter\n");

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn remove_path_entry_from_file_keeps_manual_path_entry() {
    let root = test_root("path-entry-keep-manual");
    let home = root.join("home");
    let bin_dir = home.join(".local").join("bin");
    let startup = home.join(".zshrc");

    fs::create_dir_all(&home).expect("create home");
    fs::write(
        &startup,
        format!("export PATH=\"{}:$PATH\"\n", bin_dir.display()),
    )
    .expect("manual path");

    let changed = remove_path_entry_from_file(&startup, &home, &bin_dir).expect("remove block");
    let contents = fs::read_to_string(&startup).expect("read startup");

    assert!(!changed);
    assert!(contents.contains(&bin_dir.display().to_string()));
    assert!(shell_path_entry_exists(&contents, &home, &bin_dir));

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn remove_path_entry_from_file_treats_missing_file_as_noop() {
    let root = test_root("path-entry-remove-missing");
    let home = root.join("home");
    let bin_dir = home.join(".local").join("bin");
    let startup = home.join(".profile");

    let changed = remove_path_entry_from_file(&startup, &home, &bin_dir).expect("missing noop");

    assert!(!changed);

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn remove_path_entry_from_file_reports_directory_read_errors() {
    let root = test_root("path-entry-remove-directory-read-error");
    let home = root.join("home");
    let bin_dir = home.join(".local").join("bin");
    let startup = home.join(".profile");

    fs::create_dir_all(&startup).expect("startup directory");

    let err = remove_path_entry_from_file(&startup, &home, &bin_dir)
        .expect_err("directory should not be treated as missing");

    assert!(err.to_string().contains("failed to read"));

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn shell_path_entry_exists_requires_export_line_with_target_path() {
    let home = std::path::PathBuf::from("/tmp/unixnotis-home");
    let bin_dir = home.join(".local").join("bin");

    assert!(!shell_path_entry_exists(
        "PATH=\"$HOME/.local/bin:$PATH\"",
        &home,
        &bin_dir
    ));
    assert!(!shell_path_entry_exists(
        "export PATH=\"/opt/other:$PATH\"",
        &home,
        &bin_dir
    ));
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

#[test]
fn format_path_for_shell_line_handles_home_and_non_home_paths() {
    let home = std::path::PathBuf::from("/tmp/unixnotis-home");

    // Home itself stays portable, while unrelated paths are left absolute
    assert_eq!(format_path_for_shell_line(&home, &home), "$HOME");
    assert_eq!(
        format_path_for_shell_line(&home, std::path::Path::new("/opt/unixnotis/bin")),
        "/opt/unixnotis/bin"
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
