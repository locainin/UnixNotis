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
fn empty_service_manager_choice_keeps_env_default_but_rejects_explicit_cli_value() {
    // Environment parsing keeps the historical fallback for an empty export
    assert_eq!(
        ServiceManagerChoice::parse("").expect("empty env fallback"),
        ServiceManagerChoice::Systemd
    );

    // CLI parsing is stricter because an empty flag value is almost always a typo
    assert!(ServiceManagerChoice::parse_explicit("").is_err());
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
