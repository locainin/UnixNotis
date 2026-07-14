use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc;
use std::time::Duration;

use notify::event::ModifyKind;
use notify::{Event, EventKind};
use unixnotis_core::ThemePaths;

use super::{event_targets_config, start_config_watcher, start_css_watcher, CssKind};

fn unique_root(label: &str) -> PathBuf {
    static NEXT_DIR: AtomicUsize = AtomicUsize::new(0);
    let serial = NEXT_DIR.fetch_add(1, Ordering::Relaxed);
    let root = std::env::temp_dir().join(format!(
        "unixnotis-css-watch-{label}-{}-{serial}",
        std::process::id()
    ));
    fs::create_dir_all(&root).expect("create watcher test directory");
    root
}

fn theme_paths(root: &std::path::Path) -> ThemePaths {
    ThemePaths {
        base_dir: root.to_path_buf(),
        base_css: root.join("base.css"),
        popup_css: root.join("popup.css"),
        panel_css: root.join("panel.css"),
        widgets_css: root.join("widgets.css"),
        media_css: root.join("media.css"),
    }
}

#[test]
fn config_event_filter_accepts_only_the_config_file_name() {
    let matching =
        Event::new(EventKind::Modify(ModifyKind::Any)).add_path(PathBuf::from("/tmp/config.toml"));
    let unrelated =
        Event::new(EventKind::Modify(ModifyKind::Any)).add_path(PathBuf::from("/tmp/panel.css"));

    assert!(event_targets_config(
        &matching,
        Some(std::ffi::OsStr::new("config.toml"))
    ));
    assert!(!event_targets_config(
        &unrelated,
        Some(std::ffi::OsStr::new("config.toml"))
    ));
    assert!(event_targets_config(&unrelated, None));
}

#[test]
fn config_watcher_ignores_neighbor_then_reports_target_write() {
    let root = unique_root("config");
    let config_path = root.join("config.toml");
    let neighbor_path = root.join("panel.css");
    fs::write(&config_path, "title = 'old'").expect("seed config file");
    let (tx, rx) = mpsc::channel();

    start_config_watcher(&config_path, move || {
        let _ = tx.send(());
    })
    .expect("start config watcher");

    fs::write(&neighbor_path, "/* unrelated */").expect("write neighboring css file");
    assert!(rx.recv_timeout(Duration::from_millis(350)).is_err());
    fs::write(&config_path, "title = 'new'").expect("update config file");
    rx.recv_timeout(Duration::from_secs(3))
        .expect("config watcher should report the target write");

    fs::remove_dir_all(root).expect("remove config watcher test directory");
}

#[test]
fn css_watcher_reports_a_registered_theme_write() {
    let root = unique_root("css");
    let paths = theme_paths(&root);
    fs::write(&paths.panel_css, ".panel { color: red; }").expect("seed panel css file");
    let (tx, rx) = mpsc::channel();

    start_css_watcher(&paths, CssKind::Panel, move || {
        let _ = tx.send(());
    })
    .expect("start css watcher");

    fs::write(&paths.panel_css, ".panel { color: blue; }").expect("update panel css file");
    rx.recv_timeout(Duration::from_secs(3))
        .expect("css watcher should report the registered theme write");

    fs::remove_dir_all(root).expect("remove css watcher test directory");
}
