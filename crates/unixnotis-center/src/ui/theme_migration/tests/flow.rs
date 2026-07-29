//! End-to-end migration notice state tests

use std::fs;
use std::io::Read;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, OnceLock};

use gtk::prelude::*;
use unixnotis_core::{detect_stock_theme_migration, Config, ThemePaths, DEFAULT_PANEL_CSS};
use unixnotis_ui::css::CssManager;

use crate::control::{UiCommand, UiEvent};
use crate::ui::{UiState, UiStateInit};

static LEGACY_PANEL_CSS: OnceLock<Vec<u8>> = OnceLock::new();
static APP_ID: AtomicUsize = AtomicUsize::new(0);

fn legacy_panel_css() -> &'static [u8] {
    LEGACY_PANEL_CSS
        .get_or_init(|| {
            let compressed = include_bytes!("fixtures/legacy-panel-9ca42584.css.gz");
            let mut decoder = flate2::read::GzDecoder::new(compressed.as_slice());
            let mut css = Vec::new();
            decoder
                .read_to_end(&mut css)
                .expect("historical panel fixture should decompress");
            css
        })
        .as_slice()
}

struct MigrationFixture {
    state: UiState,
    paths: ThemePaths,
    root: PathBuf,
}

impl Drop for MigrationFixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.root).expect("migration test directory should be removed");
    }
}

fn migration_fixture(name: &str) -> MigrationFixture {
    let serial = APP_ID.fetch_add(1, Ordering::Relaxed);
    let app = gtk::Application::builder()
        .application_id(format!("dev.unixnotis.theme.migration.test{serial}"))
        .flags(gtk::gio::ApplicationFlags::NON_UNIQUE)
        .build();
    app.register(None::<&gtk::gio::Cancellable>)
        .expect("test application should register");

    let mut config = Config::default();
    // Optional processes stay outside the migration fixture
    config.panel.respect_work_area = false;
    config.media.enabled = false;
    config.widgets.volume.enabled = false;
    config.widgets.brightness.enabled = false;
    config.widgets.toggles.clear();
    config.widgets.stats.clear();
    config.widgets.cards.clear();

    let root = std::env::current_dir()
        .expect("current directory should resolve")
        .join("target")
        .join(format!(
            "unixnotis-theme-migration-{name}-{}-{serial}",
            std::process::id()
        ));
    fs::create_dir_all(&root).expect("migration test directory should be created");
    let paths = config
        .resolve_theme_paths_from(&root)
        .expect("theme paths should resolve");
    fs::write(&paths.panel_css, legacy_panel_css()).expect("legacy panel CSS should be written");
    config
        .ensure_theme_files(&paths)
        .expect("active and staged theme files should be prepared");

    let css = CssManager::new_panel(paths.clone(), config.theme.clone());
    let (command_tx, _command_rx) = tokio::sync::mpsc::channel::<UiCommand>(8);
    let (event_tx, _event_rx) = async_channel::bounded::<UiEvent>(8);
    let runtime = Arc::new(tokio::runtime::Runtime::new().expect("test runtime should build"));
    let state = UiState::new(UiStateInit {
        app,
        config,
        config_path: root.join("config.toml"),
        command_tx,
        css,
        event_tx,
        media_handle: None,
        runtime,
    });

    MigrationFixture { state, paths, root }
}

#[gtk::test]
fn startup_offers_actions_for_an_exact_historical_stock_theme() {
    let fixture = migration_fixture("startup");

    assert!(
        fixture.state.theme_migration.is_some(),
        "the exact historical panel should produce a migration plan"
    );
    assert!(
        fixture.state.panel.reload_notice.revealer.reveals_child(),
        "the migration notice should be visible at startup"
    );
    assert!(
        fixture.state.panel.reload_notice.actions.get_visible(),
        "Preview, Apply, and Keep Current should be visible"
    );
    assert!(
        !fixture.state.panel.reload_notice.close.get_visible(),
        "the generic close action must not bypass the explicit choice"
    );
    assert!(
        fixture
            .state
            .panel
            .reload_notice
            .label
            .text()
            .contains("panel"),
        "the notice should identify the eligible layer"
    );
}

#[gtk::test]
fn preview_event_uses_verified_staged_css_without_changing_the_active_file() {
    let mut fixture = migration_fixture("preview");

    fixture.state.handle_event(UiEvent::ThemeMigrationPreview);

    assert!(
        fixture.state.theme_preview_active,
        "Preview should mark the in-memory CSS state active"
    );
    assert_ne!(
        fixture.state.css.theme_paths().panel_css,
        fixture.paths.panel_css,
        "Preview should point the provider at a versioned stock sibling"
    );
    assert_eq!(
        fs::read(&fixture.paths.panel_css).expect("active panel CSS should remain readable"),
        legacy_panel_css(),
        "Preview must not replace the user-editable active file"
    );
    assert!(
        fixture
            .state
            .panel
            .reload_notice
            .label
            .text()
            .contains("preview active"),
        "the notice should explain the temporary preview state"
    );
}

#[gtk::test]
fn apply_replaces_exact_stock_only_after_click_and_clears_the_notice() {
    let mut fixture = migration_fixture("apply");

    fixture.state.apply_stock_theme_migration();

    assert!(
        fixture.state.theme_migration.is_none(),
        "a successful Apply should consume the plan"
    );
    assert!(!fixture.state.theme_preview_active);
    assert_eq!(
        fixture.state.css.theme_paths().panel_css,
        fixture.paths.panel_css,
        "Apply should restore the configured active path"
    );
    assert_eq!(
        fs::read(&fixture.paths.panel_css).expect("applied panel CSS should be readable"),
        DEFAULT_PANEL_CSS.as_bytes(),
        "Apply should publish current stock bytes"
    );
    assert!(
        !fixture.state.panel.reload_notice.revealer.reveals_child(),
        "the migration notice should close after a successful Apply"
    );
    assert!(
        fs::read_dir(&fixture.root)
            .expect("theme directory should remain readable")
            .filter_map(Result::ok)
            .any(|entry| entry.file_name().to_string_lossy().ends_with(".bak")),
        "Apply should retain a recoverable backup"
    );
}

#[gtk::test]
fn keep_current_restores_a_preview_and_persists_the_choice() {
    let mut fixture = migration_fixture("keep");
    fixture.state.preview_stock_theme_migration();

    fixture.state.keep_current_stock_theme();

    assert!(fixture.state.theme_migration.is_none());
    assert!(!fixture.state.theme_preview_active);
    assert_eq!(
        fixture.state.css.theme_paths().panel_css,
        fixture.paths.panel_css,
        "Keep Current should restore the configured active path"
    );
    assert_eq!(
        fs::read(&fixture.paths.panel_css).expect("kept panel CSS should be readable"),
        legacy_panel_css(),
        "Keep Current must preserve the historical bytes"
    );
    assert!(
        detect_stock_theme_migration(&fixture.paths)
            .expect("persisted choice should remain readable")
            .is_none(),
        "the version-scoped choice should suppress the same notice on restart"
    );
}

#[gtk::test]
fn stale_apply_reports_failure_and_preserves_the_newer_edit() {
    let mut fixture = migration_fixture("stale-apply");
    let edited = b"/* edited after the notice */\n";
    fs::write(&fixture.paths.panel_css, edited).expect("newer edit should be written");

    fixture.state.apply_stock_theme_migration();

    assert!(
        fixture.state.theme_migration.is_some(),
        "a failed Apply should retain an explicit recovery choice"
    );
    assert!(fixture.state.panel.reload_notice.actions.get_visible());
    assert!(
        fixture
            .state
            .panel
            .reload_notice
            .label
            .text()
            .contains("stopped safely"),
        "the panel should explain that no stale approval was used"
    );
    assert_eq!(
        fs::read(&fixture.paths.panel_css).expect("edited panel CSS should remain readable"),
        edited,
        "the newer edit must remain active"
    );
}
