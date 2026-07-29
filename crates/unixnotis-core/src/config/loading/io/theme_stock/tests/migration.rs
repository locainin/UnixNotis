//! Explicit stock migration policy tests

use std::fs;

use crate::{Config, DEFAULT_PANEL_CSS};

use super::super::super::{ConfigError, ThemePaths};
use super::super::files::{
    collision_candidate, inspect_stock_file, stock_backup_path, stock_keep_marker_path,
};
use super::super::migration::{
    apply_stock_theme_migration, detect_stock_theme_migration,
    detect_stock_theme_migration_with_specs, keep_current_stock_theme,
    replace_file_if_snapshot_matches, LegacyThemeSpec,
};
use super::super::model::{StockThemeLayer, StockThemeMigration};
use super::super::staging::{stage_current_stock_themes, stage_stock_preview};
use super::super::MAX_STOCK_THEME_BYTES;
use super::test_root;

const LEGACY_STOCK: &[u8] = b"/* exact previous stock */\n.card { color: red; }\n";

fn detect_panel_migration(paths: &ThemePaths) -> Result<Option<StockThemeMigration>, ConfigError> {
    let digest = blake3::hash(LEGACY_STOCK).to_hex().to_string();
    detect_stock_theme_migration_with_specs(
        paths,
        &[LegacyThemeSpec {
            layer: StockThemeLayer::Panel,
            digest: &digest,
        }],
    )
}

#[test]
fn exact_legacy_stock_requires_an_explicit_apply_before_replacement() {
    let root = test_root("explicit-apply");
    fs::create_dir_all(&root).expect("theme root should be created");
    let paths = Config::default()
        .resolve_theme_paths_from(&root)
        .expect("theme paths should resolve");
    fs::write(&paths.panel_css, LEGACY_STOCK).expect("legacy stock should be written");
    stage_current_stock_themes(&paths).expect("current stock should stage");

    let migration = detect_panel_migration(&paths)
        .expect("migration detection should succeed")
        .expect("exact stock should be eligible");

    assert_eq!(
        fs::read(&paths.panel_css).expect("active stock should remain readable"),
        LEGACY_STOCK,
        "detection and startup staging must not replace the active file"
    );
    assert_eq!(migration.layer_count(), 1, "one layer should be eligible");
    assert_eq!(
        migration.fingerprint().len(),
        64,
        "the plan fingerprint should retain a full BLAKE3 identity"
    );
    assert_eq!(
        migration.layer_summary(),
        "panel",
        "the notice should name the eligible layer"
    );

    let report = apply_stock_theme_migration(&paths, &migration)
        .expect("explicitly approved migration should apply");

    assert_eq!(report.updated_layers, 1, "one layer should be updated");
    assert_eq!(
        fs::read(&paths.panel_css).expect("updated stock should be readable"),
        DEFAULT_PANEL_CSS.as_bytes(),
        "Apply should publish current stock bytes"
    );
    assert_eq!(
        fs::read(stock_backup_path(&paths.panel_css).expect("backup path should resolve"))
            .expect("backup should be readable"),
        LEGACY_STOCK,
        "Apply should preserve the exact prior bytes"
    );

    fs::remove_dir_all(root).expect("theme root should be removed");
}

#[test]
fn preview_paths_change_only_the_exact_legacy_layers() {
    let root = test_root("preview-paths");
    fs::create_dir_all(&root).expect("theme root should be created");
    let paths = Config::default()
        .resolve_theme_paths_from(&root)
        .expect("theme paths should resolve");
    fs::write(&paths.panel_css, LEGACY_STOCK).expect("legacy panel should be written");
    stage_current_stock_themes(&paths).expect("stock previews should stage");
    let migration = detect_panel_migration(&paths)
        .expect("migration detection should succeed")
        .expect("exact stock should be eligible");

    let preview = migration
        .preview_paths(&paths)
        .expect("verified preview paths should resolve");

    assert_ne!(
        preview.panel_css, paths.panel_css,
        "eligible panel CSS should point to the staged preview"
    );
    assert_eq!(
        preview.widgets_css, paths.widgets_css,
        "unrelated widget CSS should retain its configured path"
    );
    assert_eq!(
        preview.media_css, paths.media_css,
        "unrelated media CSS should retain its configured path"
    );
    assert_eq!(
        preview.popup_css, paths.popup_css,
        "popup CSS should not be folded into a panel migration"
    );

    fs::remove_dir_all(root).expect("theme root should be removed");
}

#[test]
fn keep_current_persists_the_choice_without_changing_theme_bytes() {
    let root = test_root("keep-current");
    fs::create_dir_all(&root).expect("theme root should be created");
    let paths = Config::default()
        .resolve_theme_paths_from(&root)
        .expect("theme paths should resolve");
    fs::write(&paths.panel_css, LEGACY_STOCK).expect("legacy panel should be written");
    assert!(
        detect_panel_migration(&paths)
            .expect("initial detection should succeed")
            .is_some(),
        "exact legacy stock should initially produce a notice"
    );

    keep_current_stock_theme(&paths).expect("Keep Current should persist");

    assert!(
        detect_panel_migration(&paths)
            .expect("post-choice detection should succeed")
            .is_none(),
        "the current release should respect the persisted choice"
    );
    assert_eq!(
        fs::read(&paths.panel_css).expect("kept theme should be readable"),
        LEGACY_STOCK,
        "Keep Current must not alter active CSS"
    );
    assert!(
        stock_keep_marker_path(&paths.base_dir).is_file(),
        "Keep Current should create a regular release marker"
    );

    fs::remove_dir_all(root).expect("theme root should be removed");
}

#[test]
fn custom_theme_is_not_offered_as_a_stock_migration() {
    let root = test_root("custom-theme");
    fs::create_dir_all(&root).expect("theme root should be created");
    let paths = Config::default()
        .resolve_theme_paths_from(&root)
        .expect("theme paths should resolve");
    fs::write(&paths.panel_css, b"/* customized */").expect("custom panel should be written");

    let migration = detect_panel_migration(&paths).expect("custom theme inspection should succeed");

    assert!(
        migration.is_none(),
        "non-stock bytes must remain outside the migration flow"
    );
    fs::remove_dir_all(root).expect("theme root should be removed");
}

#[test]
fn stale_apply_does_not_replace_an_edit_made_after_the_notice() {
    let root = test_root("stale-apply");
    fs::create_dir_all(&root).expect("theme root should be created");
    let paths = Config::default()
        .resolve_theme_paths_from(&root)
        .expect("theme paths should resolve");
    fs::write(&paths.panel_css, LEGACY_STOCK).expect("legacy panel should be written");
    let migration = detect_panel_migration(&paths)
        .expect("migration detection should succeed")
        .expect("exact stock should be eligible");
    let edited = b"/* user edit after notice */\n";
    fs::write(&paths.panel_css, edited).expect("user edit should be written");

    let error = apply_stock_theme_migration(&paths, &migration)
        .expect_err("stale approval must be rejected");

    assert!(
        error.to_string().contains("changed"),
        "the failure should explain that the approval became stale"
    );
    assert_eq!(
        fs::read(&paths.panel_css).expect("edited theme should be readable"),
        edited,
        "the newer edit must win"
    );
    fs::remove_dir_all(root).expect("theme root should be removed");
}

#[test]
fn final_snapshot_check_preserves_a_concurrent_edit() {
    let root = test_root("final-snapshot");
    fs::create_dir_all(&root).expect("theme root should be created");
    let path = root.join("panel.css");
    fs::write(&path, LEGACY_STOCK).expect("legacy panel should be written");
    let (snapshot, _contents) =
        inspect_stock_file(&path).expect("initial snapshot should be captured");
    let edited = b"/* editor won the race */\n";
    fs::write(&path, edited).expect("concurrent edit should be written");

    let replaced = replace_file_if_snapshot_matches(&path, DEFAULT_PANEL_CSS.as_bytes(), &snapshot)
        .expect("snapshot comparison should complete");

    assert!(!replaced, "a changed file must not be replaced");
    assert_eq!(
        fs::read(&path).expect("edited theme should be readable"),
        edited,
        "the concurrent edit must remain intact"
    );
    fs::remove_dir_all(root).expect("theme root should be removed");
}

#[cfg(unix)]
#[test]
fn linked_theme_is_never_eligible_for_replacement() {
    let root = test_root("linked-theme");
    fs::create_dir_all(&root).expect("theme root should be created");
    let paths = Config::default()
        .resolve_theme_paths_from(&root)
        .expect("theme paths should resolve");
    let protected = root.join("protected.css");
    fs::write(&protected, LEGACY_STOCK).expect("protected file should be written");
    std::os::unix::fs::symlink(&protected, &paths.panel_css)
        .expect("active theme link should be created");

    let migration =
        detect_panel_migration(&paths).expect("linked theme inspection should remain non-fatal");

    assert!(migration.is_none(), "linked CSS must never become eligible");
    assert_eq!(
        fs::read(&protected).expect("protected CSS should be readable"),
        LEGACY_STOCK,
        "the link target must remain unchanged"
    );
    fs::remove_dir_all(root).expect("theme root should be removed");
}

#[test]
fn every_candidate_is_revalidated_before_the_first_replacement() {
    let root = test_root("whole-plan-revalidation");
    fs::create_dir_all(&root).expect("theme root should be created");
    let paths = Config::default()
        .resolve_theme_paths_from(&root)
        .expect("theme paths should resolve");
    fs::write(&paths.panel_css, LEGACY_STOCK).expect("legacy panel should be written");
    fs::write(&paths.widgets_css, LEGACY_STOCK).expect("legacy widgets should be written");
    let digest = blake3::hash(LEGACY_STOCK).to_hex().to_string();
    let migration = detect_stock_theme_migration_with_specs(
        &paths,
        &[
            LegacyThemeSpec {
                layer: StockThemeLayer::Panel,
                digest: &digest,
            },
            LegacyThemeSpec {
                layer: StockThemeLayer::Widgets,
                digest: &digest,
            },
        ],
    )
    .expect("migration detection should succeed")
    .expect("both exact layers should be eligible");
    assert_eq!(
        migration.layer_count(),
        2,
        "both exact layers should remain represented in the plan"
    );
    fs::write(&paths.widgets_css, b"/* later widget edit */")
        .expect("later widget edit should be written");

    apply_stock_theme_migration(&paths, &migration)
        .expect_err("one stale layer should reject the complete plan");

    assert_eq!(
        fs::read(&paths.panel_css).expect("panel should remain readable"),
        LEGACY_STOCK,
        "a later stale layer must stop earlier candidates from being replaced"
    );
    assert_eq!(
        fs::read(&paths.widgets_css).expect("widgets should remain readable"),
        b"/* later widget edit */",
        "the newer widget edit must remain intact"
    );
    fs::remove_dir_all(root).expect("theme root should be removed");
}

#[test]
fn conflicting_backup_is_preserved_and_apply_uses_a_suffix() {
    let root = test_root("backup-collision");
    fs::create_dir_all(&root).expect("theme root should be created");
    let paths = Config::default()
        .resolve_theme_paths_from(&root)
        .expect("theme paths should resolve");
    fs::write(&paths.panel_css, LEGACY_STOCK).expect("legacy panel should be written");
    let backup = stock_backup_path(&paths.panel_css).expect("backup path should resolve");
    fs::write(&backup, b"/* unrelated backup */").expect("collision should be written");
    let migration = detect_panel_migration(&paths)
        .expect("migration detection should succeed")
        .expect("exact stock should be eligible");

    apply_stock_theme_migration(&paths, &migration)
        .expect("Apply should use a collision-safe backup name");

    assert_eq!(
        fs::read(&backup).expect("collision should remain readable"),
        b"/* unrelated backup */",
        "Apply must never overwrite an existing backup"
    );
    assert_eq!(
        fs::read(collision_candidate(&backup, 1)).expect("suffix backup should be readable"),
        LEGACY_STOCK,
        "the exact prior bytes should use the next available suffix"
    );
    fs::remove_dir_all(root).expect("theme root should be removed");
}

#[test]
fn modified_staged_file_cannot_be_loaded_as_a_stock_preview() {
    let root = test_root("tampered-preview");
    fs::create_dir_all(&root).expect("theme root should be created");
    let paths = Config::default()
        .resolve_theme_paths_from(&root)
        .expect("theme paths should resolve");
    fs::write(&paths.panel_css, LEGACY_STOCK).expect("legacy panel should be written");
    let staged = stage_stock_preview(&paths.panel_css, DEFAULT_PANEL_CSS.as_bytes())
        .expect("panel preview should stage");
    let migration = detect_panel_migration(&paths)
        .expect("migration detection should succeed")
        .expect("exact stock should be eligible");
    fs::write(&staged, b"/* changed after staging */").expect("staged file should be changed");

    migration
        .preview_paths(&paths)
        .expect_err("changed staged bytes must not be loaded as stock");

    assert_eq!(
        fs::read(&paths.panel_css).expect("active panel should remain readable"),
        LEGACY_STOCK,
        "a failed preview must not change the active theme"
    );
    fs::remove_dir_all(root).expect("theme root should be removed");
}

#[cfg(unix)]
#[test]
fn keep_current_rejects_a_marker_symlink_without_touching_its_target() {
    let root = test_root("linked-keep-marker");
    fs::create_dir_all(&root).expect("theme root should be created");
    let paths = Config::default()
        .resolve_theme_paths_from(&root)
        .expect("theme paths should resolve");
    let protected = root.join("protected-choice.txt");
    fs::write(&protected, b"protected").expect("protected marker target should be written");
    std::os::unix::fs::symlink(&protected, stock_keep_marker_path(&paths.base_dir))
        .expect("marker link should be created");

    keep_current_stock_theme(&paths).expect_err("a linked marker must be rejected");

    assert_eq!(
        fs::read(&protected).expect("protected target should remain readable"),
        b"protected",
        "Keep Current must never follow a marker link"
    );
    fs::remove_dir_all(root).expect("theme root should be removed");
}

#[test]
fn changed_configured_path_rejects_the_original_plan() {
    let root = test_root("changed-config-path");
    fs::create_dir_all(&root).expect("theme root should be created");
    let paths = Config::default()
        .resolve_theme_paths_from(&root)
        .expect("theme paths should resolve");
    fs::write(&paths.panel_css, LEGACY_STOCK).expect("legacy panel should be written");
    stage_current_stock_themes(&paths).expect("stock previews should stage");
    let migration = detect_panel_migration(&paths)
        .expect("migration detection should succeed")
        .expect("exact stock should be eligible");
    let mut changed_paths = paths.clone();
    changed_paths.panel_css = root.join("different-panel.css");

    migration
        .preview_paths(&changed_paths)
        .expect_err("a plan must stay bound to its configured path");
    apply_stock_theme_migration(&changed_paths, &migration)
        .expect_err("Apply must reject a changed configured path");

    assert_eq!(
        fs::read(&paths.panel_css).expect("original panel should remain readable"),
        LEGACY_STOCK,
        "path drift must not alter the originally approved file"
    );
    fs::remove_dir_all(root).expect("theme root should be removed");
}

#[test]
fn stock_file_inspection_accepts_the_exact_limit_and_rejects_one_more_byte() {
    let root = test_root("inspection-size-boundary");
    fs::create_dir_all(&root).expect("theme root should be created");
    let exact_size = usize::try_from(MAX_STOCK_THEME_BYTES).expect("test limit should fit usize");
    let exact = root.join("exact.css");
    let oversized = root.join("oversized.css");
    fs::write(&exact, vec![b'x'; exact_size]).expect("exact-sized CSS should be written");
    fs::write(&oversized, vec![b'x'; exact_size.saturating_add(1)])
        .expect("oversized CSS should be written");

    let (snapshot, contents) =
        inspect_stock_file(&exact).expect("the exact size limit should be accepted");
    let error = inspect_stock_file(&oversized).expect_err("one extra byte should be rejected");

    assert_eq!(
        snapshot.size, MAX_STOCK_THEME_BYTES,
        "the exact boundary should preserve its full size"
    );
    assert_eq!(
        contents.len(),
        exact_size,
        "the exact boundary should preserve every byte"
    );
    assert_eq!(
        error.kind(),
        std::io::ErrorKind::InvalidData,
        "oversized stock input should fail as invalid data"
    );
    fs::remove_dir_all(root).expect("theme root should be removed");
}

#[cfg(unix)]
#[test]
fn production_detection_fails_closed_for_a_linked_keep_marker() {
    let root = test_root("production-linked-marker");
    fs::create_dir_all(&root).expect("theme root should be created");
    let paths = Config::default()
        .resolve_theme_paths_from(&root)
        .expect("theme paths should resolve");
    let protected = root.join("protected-marker.txt");
    fs::write(&protected, b"protected").expect("protected marker should be written");
    std::os::unix::fs::symlink(&protected, stock_keep_marker_path(&paths.base_dir))
        .expect("linked marker should be created");

    detect_stock_theme_migration(&paths)
        .expect_err("production detection must reject an unsafe marker shape");

    assert_eq!(
        fs::read(&protected).expect("protected marker should remain readable"),
        b"protected",
        "detection must not follow or modify the marker link"
    );
    fs::remove_dir_all(root).expect("theme root should be removed");
}
