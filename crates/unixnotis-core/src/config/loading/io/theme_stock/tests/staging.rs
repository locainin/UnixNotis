//! Versioned preview staging tests

use std::fs;

use crate::{Config, DEFAULT_MEDIA_CSS, DEFAULT_PANEL_CSS, DEFAULT_POPUP_CSS, DEFAULT_WIDGETS_CSS};

use super::super::files::{collision_candidate, stock_preview_path};
use super::super::staging::{find_exact_stock_preview, stage_current_stock_themes};
use super::test_root;

#[test]
fn staging_writes_every_stock_theme_under_a_versioned_sibling_name() {
    let root = test_root("stage-all");
    fs::create_dir_all(&root).expect("theme root should be created");
    let paths = Config::default()
        .resolve_theme_paths_from(&root)
        .expect("theme paths should resolve");

    stage_current_stock_themes(&paths).expect("stock themes should stage");

    for (path, expected) in [
        (&paths.panel_css, DEFAULT_PANEL_CSS),
        (&paths.popup_css, DEFAULT_POPUP_CSS),
        (&paths.widgets_css, DEFAULT_WIDGETS_CSS),
        (&paths.media_css, DEFAULT_MEDIA_CSS),
    ] {
        let preview = stock_preview_path(path).expect("versioned stock path should resolve");
        assert_eq!(
            fs::read_to_string(preview).expect("staged stock theme should be readable"),
            expected,
            "each preview should contain the current embedded stock layer"
        );
        assert!(
            !path.exists(),
            "staging must not create or replace active CSS"
        );
    }

    fs::remove_dir_all(root).expect("theme root should be removed");
}

#[test]
fn conflicting_preview_is_preserved_and_exact_stock_uses_a_suffix() {
    let root = test_root("preview-collision");
    fs::create_dir_all(&root).expect("theme root should be created");
    let paths = Config::default()
        .resolve_theme_paths_from(&root)
        .expect("theme paths should resolve");
    let primary = stock_preview_path(&paths.panel_css).expect("preview path should resolve");
    fs::write(&primary, "/* reviewed custom file */").expect("collision should be written");

    stage_current_stock_themes(&paths).expect("stock themes should stage around collisions");

    let fallback = collision_candidate(&primary, 1);
    assert_eq!(
        fs::read_to_string(&primary).expect("collision should remain readable"),
        "/* reviewed custom file */",
        "staging must preserve an occupied versioned path"
    );
    assert_eq!(
        fs::read_to_string(&fallback).expect("fallback preview should be readable"),
        DEFAULT_PANEL_CSS,
        "a collision-safe sibling should contain exact stock bytes"
    );
    assert_eq!(
        find_exact_stock_preview(&paths.panel_css, DEFAULT_PANEL_CSS.as_bytes())
            .expect("exact preview should be found"),
        fallback,
        "preview selection must ignore the caller-controlled collision"
    );

    fs::remove_dir_all(root).expect("theme root should be removed");
}

#[test]
fn preview_path_rejects_a_path_without_a_file_name() {
    let error = stock_preview_path(std::path::Path::new("/"))
        .expect_err("a directory root cannot identify a theme file");

    assert!(
        error.to_string().contains("no file name"),
        "the error should explain why staging cannot continue"
    );
}
