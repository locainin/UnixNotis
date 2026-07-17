use std::path::Path;

use super::super::super::test_support::configure_command_test_root;
use super::{
    build_command, build_tokio_command, command_config_dir, command_path_escapes_root,
    log_shell_fallback_once, resolve_simple_program_from_root, set_command_config_dir,
    shell_fallback_cache, shell_fallback_hash, SHELL_FALLBACK_CACHE_LIMIT,
};

#[test]
fn resolve_simple_program_roots_relative_script_paths_in_config_dir() {
    let config_dir = Path::new("/tmp/demo/unixnotis");

    assert_eq!(
        resolve_simple_program_from_root(Some(config_dir), "scripts/demo-widget"),
        config_dir.join("scripts/demo-widget")
    );
}

#[test]
fn resolve_simple_program_uses_supplied_config_dir_for_relative_scripts() {
    let config_dir = Path::new("/tmp/unixnotis-custom-config-root");

    assert_eq!(
        resolve_simple_program_from_root(Some(config_dir), "scripts/unixnotis-blue-light-state"),
        config_dir.join("scripts/unixnotis-blue-light-state")
    );
}

#[test]
fn resolve_simple_program_roots_dot_and_explicit_relative_paths() {
    let config_dir = Path::new("/tmp/demo/unixnotis");

    assert_eq!(
        resolve_simple_program_from_root(Some(config_dir), "."),
        config_dir
    );
    assert_eq!(
        resolve_simple_program_from_root(Some(config_dir), "./scripts/probe"),
        config_dir.join("./scripts/probe")
    );
}

#[test]
fn resolve_simple_program_blocks_parent_traversal_paths() {
    let config_dir = Path::new("/tmp/demo/unixnotis");

    assert_eq!(
        resolve_simple_program_from_root(Some(config_dir), "../outside-script"),
        config_dir.join(".unixnotis-blocked-command-path")
    );
}

#[test]
fn nested_parent_traversal_is_detected_after_normal_components() {
    let config_dir = Path::new("/tmp/demo/unixnotis");
    let candidate = config_dir.join("scripts/../../outside-script");

    assert!(command_path_escapes_root(config_dir, &candidate));
}

#[test]
fn command_config_root_reports_matching_and_conflicting_initialization() {
    configure_command_test_root();
    let active_root = command_config_dir().expect("resolve active command root");

    assert!(set_command_config_dir(active_root.clone()));
    assert!(!set_command_config_dir(active_root.join("different-root")));
}

#[test]
fn shell_fallback_state_is_stable_deduplicated_and_bounded() {
    let cache = shell_fallback_cache();
    assert!(std::ptr::eq(cache, shell_fallback_cache()));

    let unique = format!("printf builder-cache-{} | true", std::process::id());
    assert!(log_shell_fallback_once(&unique));
    assert!(!log_shell_fallback_once(&unique));

    for index in 0..=SHELL_FALLBACK_CACHE_LIMIT {
        let _ = log_shell_fallback_once(&format!(
            "printf builder-cache-{}-{index} | true",
            std::process::id()
        ));
    }
    let length = cache
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .len();
    assert!(length <= SHELL_FALLBACK_CACHE_LIMIT);
}

#[test]
fn shell_fallback_hash_distinguishes_command_text() {
    let first = shell_fallback_hash("printf first | true");
    let second = shell_fallback_hash("printf second | true");

    assert_ne!(first, second);
    assert_ne!(first, 0);
    assert_ne!(second, 1);
}

#[test]
fn direct_commands_use_the_config_directory_as_their_working_directory() {
    configure_command_test_root();
    let config_dir = command_config_dir().expect("resolve command config directory");
    let command = build_command("true");

    assert_eq!(command.get_current_dir(), Some(config_dir.as_path()));
}

#[test]
fn shell_fallback_commands_use_the_config_directory_as_their_working_directory() {
    configure_command_test_root();
    let config_dir = command_config_dir().expect("resolve command config directory");
    let command = build_command(". ./lib/common.sh");

    assert_eq!(command.get_current_dir(), Some(config_dir.as_path()));
}

#[test]
fn tokio_commands_use_the_config_directory_as_their_working_directory() {
    configure_command_test_root();
    let config_dir = command_config_dir().expect("resolve command config directory");
    let command = build_tokio_command("true");

    assert_eq!(
        command.as_std().get_current_dir(),
        Some(config_dir.as_path())
    );
}

#[test]
fn loader_environment_and_command_cwd_share_the_same_config_root() {
    configure_command_test_root();
    let config_dir = command_config_dir().expect("resolve command config directory");
    let command = build_command("LD_PRELOAD=./assets/libprobe.so scripts/probe");
    let preload = command
        .get_envs()
        .find(|(name, _)| *name == "LD_PRELOAD")
        .and_then(|(_, value)| value)
        .expect("LD_PRELOAD assignment");

    assert_eq!(command.get_current_dir(), Some(config_dir.as_path()));
    assert_eq!(preload, "./assets/libprobe.so");
}

#[test]
fn tokio_loader_environment_and_command_cwd_share_the_same_config_root() {
    configure_command_test_root();
    let config_dir = command_config_dir().expect("resolve command config directory");
    let command = build_tokio_command("LD_PRELOAD=./assets/libprobe.so scripts/probe");
    let command = command.as_std();
    let preload = command
        .get_envs()
        .find(|(name, _)| *name == "LD_PRELOAD")
        .and_then(|(_, value)| value)
        .expect("LD_PRELOAD assignment");

    assert_eq!(command.get_current_dir(), Some(config_dir.as_path()));
    assert_eq!(preload, "./assets/libprobe.so");
}
