use std::fs;
#[cfg(unix)]
use std::os::unix::fs::symlink;
use std::sync::atomic::AtomicBool;
use std::sync::{mpsc, Arc, Mutex, MutexGuard, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use super::super::{
    ensure_path_entry_in_file, format_path_for_shell_line, remove_path_entry_from_file,
    remove_shell_path_entry, shell_path_entry_exists, shell_startup_files,
};
use crate::actions::ActionContext;
use crate::detect::Detection;
use crate::events::{UiMessage, WorkerEvent};
use crate::model::ActionMode;
use crate::paths::InstallPaths;
use crate::service_manager::ServiceManager;

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

    assert!(err.to_string().contains("refusing to overwrite non-file"));

    let _ = fs::remove_dir_all(&root);
}

#[cfg(unix)]
#[test]
fn ensure_path_entry_in_file_rejects_startup_symlink_without_touching_target() {
    let root = test_root("path-entry-symlink-rejected");
    let home = root.join("home");
    let bin_dir = home.join(".local").join("bin");
    let startup = home.join(".profile");
    let protected = root.join("protected");

    fs::create_dir_all(&home).expect("create home");
    fs::write(&protected, "protected").expect("protected");
    symlink(&protected, &startup).expect("startup symlink");

    let err = ensure_path_entry_in_file(&startup, &home, &bin_dir)
        .expect_err("startup symlink should be rejected");

    assert!(err
        .to_string()
        .contains("refusing to write through symlink"));
    assert_eq!(
        fs::read_to_string(&protected).expect("protected remains"),
        "protected"
    );

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
fn remove_shell_path_entry_removes_managed_block_from_selected_startup_files() {
    let _lock = env_lock();
    let root = test_root("path-entry-remove-high-level");
    let home = root.join("home");
    let bin_dir = home.join(".local").join("bin");
    let startup = home.join(".bashrc");
    fs::create_dir_all(&home).expect("create home");
    ensure_path_entry_in_file(&startup, &home, &bin_dir).expect("managed block");
    let _home = EnvGuard::set("HOME", &home);
    let _shell = EnvGuard::set("SHELL", "/bin/bash");
    let (tx, rx) = mpsc::sync_channel::<UiMessage>(16);
    let detection = Detection {
        owner: None,
        daemons: Vec::new(),
    };
    let paths = InstallPaths {
        repo_root: root.clone(),
        bin_dir,
        service: ServiceManager::systemd_user(home.join(".config/systemd/user")),
    };
    let mut ctx = ActionContext {
        detection: &detection,
        paths: &paths,
        install_state: None,
        log_tx: tx,
        action_mode: ActionMode::Uninstall,
        restore_backup: None,
        service_reload_required: Arc::new(AtomicBool::new(false)),
    };

    remove_shell_path_entry(&mut ctx).expect("remove shell path entry");

    let contents = fs::read_to_string(&startup).expect("read startup");
    assert!(!contents.contains("# unixnotis-installer path entry"));
    let logs = rx.try_iter().collect::<Vec<_>>();
    assert!(logs.iter().any(|message| matches!(
        message,
        UiMessage::Worker(WorkerEvent::LogLine(line))
            if line.contains("Removed installer-owned PATH entry")
    )));

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

    assert!(err.to_string().contains("refusing to overwrite non-file"));

    let _ = fs::remove_dir_all(&root);
}

#[cfg(unix)]
#[test]
fn remove_path_entry_from_file_rejects_startup_symlink_without_touching_target() {
    let root = test_root("path-entry-remove-symlink-rejected");
    let home = root.join("home");
    let bin_dir = home.join(".local").join("bin");
    let startup = home.join(".profile");
    let protected = root.join("protected");
    let managed = "# unixnotis-installer path entry\nexport PATH=\"$HOME/.local/bin:$PATH\"\n";

    fs::create_dir_all(&home).expect("create home");
    fs::write(&protected, managed).expect("protected");
    symlink(&protected, &startup).expect("startup symlink");

    let err = remove_path_entry_from_file(&startup, &home, &bin_dir)
        .expect_err("startup symlink should be rejected");

    assert!(err
        .to_string()
        .contains("refusing to write through symlink"));
    assert_eq!(
        fs::read_to_string(&protected).expect("protected remains"),
        managed
    );

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

struct EnvGuard {
    name: &'static str,
    previous: Option<std::ffi::OsString>,
}

impl EnvGuard {
    fn set(name: &'static str, value: impl AsRef<std::ffi::OsStr>) -> Self {
        let previous = std::env::var_os(name);
        std::env::set_var(name, value);
        Self { name, previous }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        match &self.previous {
            Some(value) => std::env::set_var(self.name, value),
            None => std::env::remove_var(self.name),
        }
    }
}

fn env_lock() -> MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .expect("environment test lock")
}
