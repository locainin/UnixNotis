use super::*;

#[test]
fn format_with_home_rewrites_prefix() {
    // Confirms home-prefixed paths are rendered with the $HOME shorthand
    let Ok(home) = env::var("HOME") else {
        return;
    };
    if home.is_empty() {
        return;
    }
    let path = PathBuf::from(&home).join(".config").join("unixnotis");
    let rendered = format_with_home(&path);
    assert!(rendered.starts_with("$HOME"));
}

#[test]
fn is_unixnotis_repo_detects_markers() {
    // Validates that known workspace markers are detected in a Cargo.toml file
    let Ok(home) = env::var("HOME") else {
        return;
    };
    if home.is_empty() {
        return;
    }
    let dir = PathBuf::from(home)
        .join(".cache")
        .join(format!("unixnotis-test-{}", std::process::id()));
    if fs::create_dir_all(&dir).is_err() {
        return;
    }
    let cargo_path = dir.join("Cargo.toml");
    let contents = r#"
[package]
name = "unixnotis-daemon"

[workspace]
members = ["crates/unixnotis-daemon", "crates/unixnotis-core"]
"#;
    if fs::write(&cargo_path, contents).is_err() {
        let _ = fs::remove_dir_all(&dir);
        return;
    }

    assert!(is_unixnotis_repo(&cargo_path));
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn repo_detection_rejects_member_crate_manifest() {
    let root = env::temp_dir().join(format!(
        "unixnotis-member-crate-reject-{}",
        std::process::id()
    ));
    fs::create_dir_all(&root).expect("test root");
    let cargo_path = root.join("Cargo.toml");
    fs::write(
        &cargo_path,
        r#"
[package]
name = "unixnotis-daemon"
version = "0.1.0"
"#,
    )
    .expect("member Cargo.toml");

    // Package names are not enough; trial mode needs the workspace root for target/debug paths
    assert!(!is_unixnotis_repo(&cargo_path));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn repo_detection_requires_each_workspace_marker() {
    let root = env::temp_dir().join(format!(
        "unixnotis-workspace-marker-check-{}",
        std::process::id()
    ));
    fs::create_dir_all(&root).expect("test root");
    let cargo_path = root.join("Cargo.toml");
    for contents in [
        "[workspace]\nmembers = [\"crates/unixnotis-daemon\"]\n",
        "[workspace]\nmembers = [\"crates/unixnotis-core\"]\n",
        "[package]\nname = \"crates/unixnotis-daemon crates/unixnotis-core\"\n",
    ] {
        fs::write(&cargo_path, contents).expect("incomplete workspace manifest");
        assert!(!is_unixnotis_repo(&cargo_path), "{contents:?}");
    }

    let _ = fs::remove_dir_all(root);
}

#[test]
fn release_archive_detection_requires_manifest_and_bundled_binaries() {
    let root = env::temp_dir().join(format!(
        "unixnotis-release-archive-detect-{}",
        std::process::id()
    ));
    let bin_dir = root.join(RELEASE_BIN_DIR);
    fs::create_dir_all(&bin_dir).expect("release bin dir");
    fs::write(
        root.join(RELEASE_MANIFEST_FILE),
        r#"{"version":"1.0.0","binaries":["unixnotis-daemon","unixnotis-popups","unixnotis-center","noticenterctl"]}"#,
    )
    .expect("release manifest");

    for binary in [
        "unixnotis-daemon",
        "unixnotis-popups",
        "unixnotis-center",
        "noticenterctl",
    ] {
        fs::write(bin_dir.join(binary), format!("binary:{binary}")).expect("release binary");
    }

    assert!(is_unixnotis_release_archive(&root));

    fs::remove_file(bin_dir.join("noticenterctl")).expect("remove required binary");
    assert!(!is_unixnotis_release_archive(&root));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn release_archive_detection_uses_manifest_binary_list_without_hardcoded_names() {
    let root = env::temp_dir().join(format!(
        "unixnotis-release-archive-custom-{}",
        std::process::id()
    ));
    let bin_dir = root.join(RELEASE_BIN_DIR);
    fs::create_dir_all(&bin_dir).expect("release bin dir");
    fs::write(
        root.join(RELEASE_MANIFEST_FILE),
        r#"{"version":"1.0.0","binaries":["unixnotis-daemon"]}"#,
    )
    .expect("release manifest");
    fs::write(bin_dir.join("unixnotis-daemon"), "binary").expect("release binary");

    assert!(is_unixnotis_release_archive(&root));

    fs::remove_file(bin_dir.join("unixnotis-daemon")).expect("remove manifest binary");
    assert!(!is_unixnotis_release_archive(&root));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn release_archive_detection_rejects_unsafe_manifest_binary_names() {
    let root = env::temp_dir().join(format!(
        "unixnotis-release-archive-unsafe-{}",
        std::process::id()
    ));
    let bin_dir = root.join(RELEASE_BIN_DIR);
    fs::create_dir_all(&bin_dir).expect("release bin dir");
    fs::write(
        root.join(RELEASE_MANIFEST_FILE),
        r#"{"version":"1.0.0","binaries":["../unixnotis-daemon"]}"#,
    )
    .expect("release manifest");

    assert!(!is_unixnotis_release_archive(&root));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn release_root_discovery_prefers_explicit_archive_override() {
    let _guard = env_lock();
    let root = env::temp_dir().join(format!("unixnotis-release-root-{}", std::process::id()));
    write_release_archive(&root);
    let previous_release = set_env(
        "UNIXNOTIS_RELEASE_ROOT",
        Some(root.to_string_lossy().as_ref()),
    );
    let previous_repo = set_env("UNIXNOTIS_REPO_ROOT", None);

    let discovered = InstallPaths::discover_repo_root().expect("release root");

    // Downloaded archives do not carry Cargo.toml, so release roots need their own lookup path
    assert_eq!(discovered, root);

    restore_env("UNIXNOTIS_REPO_ROOT", previous_repo);
    restore_env("UNIXNOTIS_RELEASE_ROOT", previous_release);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn release_root_from_executable_requires_a_complete_sibling_archive() {
    let root = env::temp_dir().join(format!(
        "unixnotis-release-executable-root-{}",
        std::process::id()
    ));
    let executable = root.join("unixnotis-installer");
    fs::create_dir_all(&root).expect("release root");

    assert_eq!(release_root_from_executable(&executable), None);

    write_release_archive(&root);
    assert_eq!(
        release_root_from_executable(&executable),
        Some(root.clone())
    );

    let _ = fs::remove_dir_all(root);
}

#[test]
fn repo_root_override_rejects_an_unrelated_cargo_manifest() {
    let _guard = env_lock();
    let root = env::temp_dir().join(format!(
        "unixnotis-invalid-repo-override-{}",
        std::process::id()
    ));
    fs::create_dir_all(&root).expect("override root");
    fs::write(root.join("Cargo.toml"), "[workspace]\nmembers = []\n")
        .expect("unrelated Cargo.toml");
    let previous_release = set_env("UNIXNOTIS_RELEASE_ROOT", None);
    let previous_repo = set_env("UNIXNOTIS_REPO_ROOT", Some(root.to_string_lossy().as_ref()));

    let discovered = InstallPaths::discover_repo_root();

    assert_ne!(discovered.ok().as_deref(), Some(root.as_path()));
    restore_env("UNIXNOTIS_REPO_ROOT", previous_repo);
    restore_env("UNIXNOTIS_RELEASE_ROOT", previous_release);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn repo_root_walk_skips_an_unrelated_nested_cargo_manifest() {
    let _guard = env_lock();
    let root = env::temp_dir().join(format!("unixnotis-nested-repo-walk-{}", std::process::id()));
    let nested = root.join("nested");
    fs::create_dir_all(&nested).expect("nested directory");
    fs::write(
        root.join("Cargo.toml"),
        "[workspace]\nmembers = [\"crates/unixnotis-daemon\", \"crates/unixnotis-core\"]\n",
    )
    .expect("workspace Cargo.toml");
    fs::write(nested.join("Cargo.toml"), "[workspace]\nmembers = []\n").expect("nested Cargo.toml");
    let previous_release = set_env("UNIXNOTIS_RELEASE_ROOT", None);
    let previous_repo = set_env("UNIXNOTIS_REPO_ROOT", None);
    let current_dir = CurrentDirGuard::set(&nested);

    let discovered = InstallPaths::discover_repo_root().expect("parent workspace root");

    assert_eq!(discovered, root);
    restore_env("UNIXNOTIS_REPO_ROOT", previous_repo);
    restore_env("UNIXNOTIS_RELEASE_ROOT", previous_release);
    drop(current_dir);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn service_manager_choice_accepts_cli_names() {
    assert_eq!(
        ServiceManagerChoice::parse("systemd").expect("systemd"),
        ServiceManagerChoice::Systemd
    );
    assert_eq!(
        ServiceManagerChoice::parse("dinit").expect("dinit"),
        ServiceManagerChoice::Dinit
    );
    assert_eq!(
        ServiceManagerChoice::parse("runit").expect("runit"),
        ServiceManagerChoice::Runit
    );
    assert_eq!(
        ServiceManagerChoice::parse("s6").expect("s6"),
        ServiceManagerChoice::S6
    );
}

fn write_release_archive(root: &std::path::Path) {
    let bin_dir = root.join(RELEASE_BIN_DIR);
    fs::create_dir_all(&bin_dir).expect("release bin dir");
    fs::write(
        root.join(RELEASE_MANIFEST_FILE),
        r#"{"version":"1.0.0","binaries":["unixnotis-daemon","unixnotis-popups","unixnotis-center","noticenterctl"]}"#,
    )
    .expect("release manifest");

    for binary in [
        "unixnotis-daemon",
        "unixnotis-popups",
        "unixnotis-center",
        "noticenterctl",
    ] {
        fs::write(bin_dir.join(binary), format!("binary:{binary}")).expect("release binary");
    }
}

#[test]
fn empty_service_manager_choice_keeps_env_default_but_rejects_explicit_cli_value() {
    let _guard = env_lock();
    let previous = set_env("UNIXNOTIS_SERVICE_MANAGER", Some(""));

    // Environment parsing keeps the historical fallback for an empty export
    assert_eq!(
        service_manager_choice_from_environment().expect("empty env fallback"),
        ServiceManagerChoice::Systemd
    );
    restore_env("UNIXNOTIS_SERVICE_MANAGER", previous);

    // CLI parsing is stricter because an empty flag value is almost always a typo
    assert!(ServiceManagerChoice::parse("").is_err());
}

#[test]
fn trial_repo_root_discovery_ignores_service_manager_environment() {
    let _guard = env_lock();
    let root = env::temp_dir().join(format!("unixnotis-trial-repo-root-{}", std::process::id()));
    fs::create_dir_all(&root).expect("repo root");
    fs::write(
        root.join("Cargo.toml"),
        "[workspace]\nmembers = [\"crates/unixnotis-daemon\", \"crates/unixnotis-core\"]\n",
    )
    .expect("repo Cargo.toml");
    let previous_repo = set_env("UNIXNOTIS_REPO_ROOT", Some(root.to_string_lossy().as_ref()));
    let previous_manager = set_env("UNIXNOTIS_SERVICE_MANAGER", Some("s6"));
    let previous_user = set_env("USER", None);

    let discovered = InstallPaths::discover_repo_root().expect("trial root should not need s6");

    // Trial run launches from source, so backend-specific paths must not block this lookup
    assert_eq!(discovered, root);

    restore_env("USER", previous_user);
    restore_env("UNIXNOTIS_SERVICE_MANAGER", previous_manager);
    restore_env("UNIXNOTIS_REPO_ROOT", previous_repo);
    let _ = fs::remove_dir_all(root);
}

struct CurrentDirGuard(PathBuf);

impl CurrentDirGuard {
    fn set(path: &std::path::Path) -> Self {
        let previous = env::current_dir().expect("current directory");
        env::set_current_dir(path).expect("set current directory");
        Self(previous)
    }
}

impl Drop for CurrentDirGuard {
    fn drop(&mut self) {
        env::set_current_dir(&self.0).expect("restore current directory");
    }
}
