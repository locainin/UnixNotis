use super::super::block::{
    strip_hyprland_bootstrap_block, HYPR_BOOTSTRAP_END, HYPR_BOOTSTRAP_START,
};
use crate::app::events::UiMessage;
use crate::detect::Detection;
use crate::model::ActionMode;
use crate::paths::InstallPaths;
use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::sync::{mpsc, Arc};

#[test]
fn strip_hyprland_bootstrap_block_handles_malformed_block() {
    let _lock = crate::test_support::env::test_env_lock();
    // Confirms malformed markers leave the original content intact for safe append
    let detection = Detection {
        owner: None,
        daemons: Vec::new(),
    };
    let paths = InstallPaths::discover().expect("paths should resolve in repo tests");
    let (tx, _rx) = mpsc::sync_channel::<UiMessage>(8);
    let mut ctx = crate::actions::ActionContext {
        detection: &detection,
        paths: &paths,
        install_state: None,
        log_tx: tx,
        action_mode: ActionMode::Install,
        restore_backup: None,
        service_reload_required: Arc::new(AtomicBool::new(false)),
    };
    let contents = format!("{HYPR_BOOTSTRAP_START}\nexec-once = foo\n");
    let result = strip_hyprland_bootstrap_block(&mut ctx, &contents, Path::new("hyprland.conf"));
    assert_eq!(result.stripped, contents);
    assert!(!result.block_found);
    assert!(result.malformed);
}

#[test]
fn strip_hyprland_bootstrap_block_removes_managed_block() {
    let _lock = crate::test_support::env::test_env_lock();
    // Ensures a well-formed block is removed and the remaining content is preserved
    let detection = Detection {
        owner: None,
        daemons: Vec::new(),
    };
    let paths = InstallPaths::discover().expect("paths should resolve in repo tests");
    let (tx, _rx) = mpsc::sync_channel::<UiMessage>(8);
    let mut ctx = crate::actions::ActionContext {
        detection: &detection,
        paths: &paths,
        install_state: None,
        log_tx: tx,
        action_mode: ActionMode::Install,
        restore_backup: None,
        service_reload_required: Arc::new(AtomicBool::new(false)),
    };
    let contents =
        format!("line-a\n{HYPR_BOOTSTRAP_START}\nexec-once = foo\n{HYPR_BOOTSTRAP_END}\nline-b\n");
    let result = strip_hyprland_bootstrap_block(&mut ctx, &contents, Path::new("hyprland.conf"));
    assert_eq!(result.stripped, "line-a\nline-b\n");
    assert!(result.block_found);
    assert!(!result.malformed);
}

#[test]
fn strip_hyprland_bootstrap_block_removes_all_blocks() {
    let _lock = crate::test_support::env::test_env_lock();
    let detection = Detection {
        owner: None,
        daemons: Vec::new(),
    };
    let paths = InstallPaths::discover().expect("paths should resolve in repo tests");
    let (tx, _rx) = mpsc::sync_channel::<UiMessage>(8);
    let mut ctx = crate::actions::ActionContext {
        detection: &detection,
        paths: &paths,
        install_state: None,
        log_tx: tx,
        action_mode: ActionMode::Install,
        restore_backup: None,
        service_reload_required: Arc::new(AtomicBool::new(false)),
    };
    let contents = format!(
        "line-a\n{HYPR_BOOTSTRAP_START}\nexec-once = foo\n{HYPR_BOOTSTRAP_END}\nline-b\n{HYPR_BOOTSTRAP_START}\nexec-once = bar\n{HYPR_BOOTSTRAP_END}\nline-c\n"
    );
    let result = strip_hyprland_bootstrap_block(&mut ctx, &contents, Path::new("hyprland.conf"));
    assert_eq!(result.stripped, "line-a\nline-b\nline-c\n");
    assert!(result.block_found);
    assert!(!result.malformed);
}

#[test]
fn strip_hyprland_bootstrap_block_removes_comment_prefixes_without_residue() {
    let _lock = crate::test_support::env::test_env_lock();
    let detection = Detection {
        owner: None,
        daemons: Vec::new(),
    };
    let paths = InstallPaths::discover().expect("paths should resolve in repo tests");
    let (tx, _rx) = mpsc::sync_channel::<UiMessage>(8);
    let mut ctx = crate::actions::ActionContext {
        detection: &detection,
        paths: &paths,
        install_state: None,
        log_tx: tx,
        action_mode: ActionMode::Install,
        restore_backup: None,
        service_reload_required: Arc::new(AtomicBool::new(false)),
    };
    let contents = format!(
        "-- retained\n-- {HYPR_BOOTSTRAP_START}\n-- managed\n-- {HYPR_BOOTSTRAP_END}\n-- after\n"
    );

    let result = strip_hyprland_bootstrap_block(&mut ctx, &contents, Path::new("hyprland.lua"));

    assert_eq!(result.stripped, "-- retained\n-- after\n");
    assert!(result.block_found);
    assert!(!result.malformed);
}

#[test]
fn strip_hyprland_bootstrap_block_matches_exact_hyprlang_marker_comments() {
    let _lock = crate::test_support::env::test_env_lock();
    let detection = Detection {
        owner: None,
        daemons: Vec::new(),
    };
    let paths = InstallPaths::discover().expect("paths should resolve in repo tests");
    let (tx, _rx) = mpsc::sync_channel::<UiMessage>(8);
    let mut ctx = crate::actions::ActionContext {
        detection: &detection,
        paths: &paths,
        install_state: None,
        log_tx: tx,
        action_mode: ActionMode::Install,
        restore_backup: None,
        service_reload_required: Arc::new(AtomicBool::new(false)),
    };
    let contents = format!(
        "# retained\n# {HYPR_BOOTSTRAP_START}\nexec-once = managed\n# {HYPR_BOOTSTRAP_END}\n# after\n"
    );

    let result = strip_hyprland_bootstrap_block(&mut ctx, &contents, Path::new("hyprland.conf"));

    assert_eq!(result.stripped, "# retained\n# after\n");
    assert!(result.block_found);
    assert!(!result.malformed);
}

#[test]
fn strip_hyprland_bootstrap_block_ignores_marker_text_inside_lua_strings() {
    let _lock = crate::test_support::env::test_env_lock();
    let detection = Detection {
        owner: None,
        daemons: Vec::new(),
    };
    let paths = InstallPaths::discover().expect("paths should resolve in repo tests");
    let (tx, _rx) = mpsc::sync_channel::<UiMessage>(8);
    let mut ctx = crate::actions::ActionContext {
        detection: &detection,
        paths: &paths,
        install_state: None,
        log_tx: tx,
        action_mode: ActionMode::Install,
        restore_backup: None,
        service_reload_required: Arc::new(AtomicBool::new(false)),
    };
    let contents = format!(
        "local start = \"{HYPR_BOOTSTRAP_START}\"\nlocal finish = \"{HYPR_BOOTSTRAP_END}\"\n"
    );

    let result = strip_hyprland_bootstrap_block(&mut ctx, &contents, Path::new("hyprland.lua"));

    assert_eq!(result.stripped, contents);
    assert!(!result.block_found);
    assert!(!result.malformed);
}
