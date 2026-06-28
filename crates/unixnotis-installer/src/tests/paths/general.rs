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
