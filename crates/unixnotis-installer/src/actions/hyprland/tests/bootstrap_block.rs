use super::super::{strip_hyprland_bootstrap_block, HYPR_BOOTSTRAP_END, HYPR_BOOTSTRAP_START};
use crate::detect::Detection;
use crate::events::UiMessage;
use crate::model::ActionMode;
use crate::paths::InstallPaths;
use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::sync::{mpsc, Arc};

#[test]
fn strip_hyprland_bootstrap_block_handles_malformed_block() {
    // Confirms malformed markers leave the original content intact for safe append.
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
        service_unit_reload_required: Arc::new(AtomicBool::new(false)),
    };
    let contents = format!("{start}\nexec-once = foo\n", start = HYPR_BOOTSTRAP_START);
    let result = strip_hyprland_bootstrap_block(&mut ctx, &contents, Path::new("hyprland.conf"));
    assert_eq!(result.stripped, contents);
    assert!(!result.block_found);
    assert!(result.malformed);
}

#[test]
fn strip_hyprland_bootstrap_block_removes_managed_block() {
    // Ensures a well-formed block is removed and the remaining content is preserved.
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
        service_unit_reload_required: Arc::new(AtomicBool::new(false)),
    };
    let contents = format!(
        "line-a\n{start}\nexec-once = foo\n{end}\nline-b\n",
        start = HYPR_BOOTSTRAP_START,
        end = HYPR_BOOTSTRAP_END
    );
    let result = strip_hyprland_bootstrap_block(&mut ctx, &contents, Path::new("hyprland.conf"));
    assert_eq!(result.stripped, "line-a\nline-b\n");
    assert!(result.block_found);
    assert!(!result.malformed);
}

#[test]
fn strip_hyprland_bootstrap_block_removes_all_blocks() {
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
        service_unit_reload_required: Arc::new(AtomicBool::new(false)),
    };
    let contents = format!(
        "line-a\n{start}\nexec-once = foo\n{end}\nline-b\n{start}\nexec-once = bar\n{end}\nline-c\n",
        start = HYPR_BOOTSTRAP_START,
        end = HYPR_BOOTSTRAP_END
    );
    let result = strip_hyprland_bootstrap_block(&mut ctx, &contents, Path::new("hyprland.conf"));
    assert_eq!(result.stripped, "line-a\nline-b\nline-c\n");
    assert!(result.block_found);
    assert!(!result.malformed);
}
