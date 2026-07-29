use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use gtk::prelude::*;
use unixnotis_core::{Config, ThemeContractState, ThemeMode};
use unixnotis_ui::css::CssManager;

use crate::control::{UiCommand, UiEvent};
use crate::ui::{UiState, UiStateInit};

static NEXT_APP: AtomicUsize = AtomicUsize::new(0);

struct ThemeFixture {
    state: UiState,
    custom_css: PathBuf,
    original: String,
    root: PathBuf,
}

impl Drop for ThemeFixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.root).expect("theme test directory should be removable");
    }
}

fn incompatible_theme_fixture(name: &str) -> ThemeFixture {
    let serial = NEXT_APP.fetch_add(1, Ordering::Relaxed);
    let app = gtk::Application::builder()
        .application_id(format!("dev.unixnotis.theme.compatibility.test{serial}"))
        .flags(gtk::gio::ApplicationFlags::NON_UNIQUE)
        .build();
    app.register(None::<&gtk::gio::Cancellable>)
        .expect("test application should register");

    let mut config = Config::default();
    config.theme.mode = ThemeMode::Custom;
    config.panel.respect_work_area = false;
    config.media.enabled = false;
    config.widgets.volume.enabled = false;
    config.widgets.brightness.enabled = false;
    config.widgets.toggles.clear();
    config.widgets.stats.clear();
    config.widgets.cards.clear();

    let root = std::env::temp_dir().join(format!(
        "unixnotis-theme-compatibility-{name}-{}-{serial}",
        std::process::id()
    ));
    fs::create_dir_all(&root).expect("theme test directory should be created");
    let paths = config
        .resolve_theme_paths_from(&root)
        .expect("theme paths should resolve");
    let original = "/* incompatible custom theme must be preserved */".to_string();
    fs::write(&paths.panel_css, &original).expect("custom theme should be writable");
    let config_path = root.join("config.toml");
    fs::write(
        &config_path,
        toml::to_string_pretty(&config).expect("theme config should serialize"),
    )
    .expect("theme config should be writable");

    let css = CssManager::new_panel(paths.clone(), config.theme.clone());
    let (command_tx, _command_rx) = tokio::sync::mpsc::channel::<UiCommand>(8);
    let (event_tx, _event_rx) = async_channel::bounded::<UiEvent>(8);
    let runtime = Arc::new(tokio::runtime::Runtime::new().expect("test runtime should build"));
    let state = UiState::new(UiStateInit {
        app,
        config,
        config_path,
        command_tx,
        css,
        event_tx,
        media_handle: None,
        runtime,
    });

    ThemeFixture {
        state,
        custom_css: paths.panel_css,
        original,
        root,
    }
}

#[gtk::test]
fn incompatible_theme_shows_non_mutating_stock_fallback_notice() {
    let fixture = incompatible_theme_fixture("notice");

    assert!(fixture.state.panel.reload_notice.revealer.reveals_child());
    assert!(fixture.state.panel.reload_notice.actions.get_visible());
    assert!(!fixture.state.panel.reload_notice.close.get_visible());
    assert!(fixture
        .state
        .panel
        .reload_notice
        .label
        .text()
        .contains("incompatible"));
    assert_eq!(
        fs::read_to_string(&fixture.custom_css).expect("custom theme should remain readable"),
        fixture.original
    );
}

#[gtk::test]
fn stock_action_disables_custom_reads_without_changing_custom_file() {
    let mut fixture = incompatible_theme_fixture("stock");

    fixture.state.handle_event(UiEvent::UseStockTheme);

    assert_eq!(
        fixture.state.css.theme_contract(),
        ThemeContractState::EmbeddedStock
    );
    assert!(!fixture.state.panel.reload_notice.revealer.reveals_child());
    assert_eq!(
        fs::read_to_string(&fixture.custom_css).expect("custom theme should remain readable"),
        fixture.original
    );
    let persisted = Config::load_from_path(&fixture.state.config_path)
        .expect("stock theme selection should leave a valid config");
    assert_eq!(
        persisted.theme.mode,
        ThemeMode::Stock,
        "stock theme selection must survive a process restart"
    );
    let paths = persisted
        .resolve_theme_paths_from(&fixture.root)
        .expect("persisted theme paths should resolve");
    assert_eq!(
        CssManager::new_panel(paths, persisted.theme).theme_contract(),
        ThemeContractState::EmbeddedStock,
        "a new CSS manager must retain the persisted stock selection"
    );
}
