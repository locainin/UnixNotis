//! File watcher helpers for CSS and config hot reload.

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher};
use tracing::warn;
use unixnotis_core::ThemePaths;

use super::CssKind;

/// Start a file watcher for CSS paths and emit reload callbacks.
pub fn start_css_watcher(paths: &ThemePaths, kind: CssKind, on_reload: impl Fn() + Send + 'static) {
    let mut watched_dirs = HashSet::new();
    let css_paths = match kind {
        CssKind::Panel => vec![&paths.base_css, &paths.panel_css, &paths.widgets_css],
        CssKind::Popup => vec![&paths.base_css, &paths.popup_css],
    };
    for path in css_paths {
        if let Some(dir) = path.parent() {
            watched_dirs.insert(dir.to_path_buf());
        }
    }

    if watched_dirs.is_empty() {
        return;
    }

    thread::spawn(move || {
        let (event_tx, event_rx) = mpsc::channel::<notify::Result<Event>>();
        let mut watcher = match RecommendedWatcher::new(
            move |res| {
                let _ = event_tx.send(res);
            },
            notify::Config::default(),
        ) {
            Ok(watcher) => watcher,
            Err(err) => {
                warn!(?err, "failed to create css watcher");
                return;
            }
        };

        for dir in &watched_dirs {
            if let Err(err) = watcher.watch(dir, RecursiveMode::NonRecursive) {
                warn!(?err, "failed to watch css directory");
            }
        }

        let debounce = Duration::from_millis(150);
        // Block on recv so the watcher thread does not wake periodically when idle.
        // Using recv_timeout here would wake every debounce interval and burn CPU
        // even when no files change.
        while let Ok(event) = event_rx.recv() {
            if let Err(err) = event {
                warn!(?err, "css watcher reported an error");
                continue;
            }
            // Once an event arrives, coalesce bursts by waiting for a quiet window.
            // This keeps reloads responsive while minimizing redundant reload work.
            loop {
                match event_rx.recv_timeout(debounce) {
                    Ok(event) => {
                        if let Err(err) = event {
                            warn!(?err, "css watcher reported an error");
                        }
                    }
                    Err(mpsc::RecvTimeoutError::Timeout) => break,
                    Err(mpsc::RecvTimeoutError::Disconnected) => return,
                }
            }
            on_reload();
        }
    });
}

/// Start a file watcher for the config path and emit reload callbacks.
pub fn start_config_watcher(config_path: PathBuf, on_reload: impl Fn() + Send + 'static) {
    let Some(parent) = config_path.parent().map(PathBuf::from) else {
        return;
    };
    let config_name = config_path.file_name().map(|name| name.to_os_string());
    thread::spawn(move || {
        let (event_tx, event_rx) = mpsc::channel::<notify::Result<Event>>();
        let mut watcher = match RecommendedWatcher::new(
            move |res| {
                let _ = event_tx.send(res);
            },
            notify::Config::default(),
        ) {
            Ok(watcher) => watcher,
            Err(err) => {
                warn!(?err, "failed to create config watcher");
                return;
            }
        };

        if let Err(err) = watcher.watch(&parent, RecursiveMode::NonRecursive) {
            warn!(?err, "failed to watch config directory");
        }

        let debounce = Duration::from_millis(150);
        // Block on recv so the watcher thread does not wake periodically when idle.
        // Using recv_timeout here would wake every debounce interval and burn CPU
        // even when no files change.
        while let Ok(event) = event_rx.recv() {
            let Ok(event) = event else {
                warn!(?event, "config watcher reported an error");
                continue;
            };
            if let Some(name) = config_name.as_ref() {
                let matches = event
                    .paths
                    .iter()
                    .any(|path| path.file_name() == Some(name));
                if !matches {
                    continue;
                }
            }
            // Coalesce rapid edits by draining events until the debounce window is quiet.
            // This avoids multiple reloads during a single save operation.
            loop {
                match event_rx.recv_timeout(debounce) {
                    Ok(event) => {
                        let Ok(event) = event else {
                            warn!(?event, "config watcher reported an error");
                            continue;
                        };
                        if let Some(name) = config_name.as_ref() {
                            let matches = event
                                .paths
                                .iter()
                                .any(|path| path.file_name() == Some(name));
                            if !matches {
                                continue;
                            }
                        }
                    }
                    Err(mpsc::RecvTimeoutError::Timeout) => break,
                    Err(mpsc::RecvTimeoutError::Disconnected) => return,
                }
            }
            on_reload();
        }
    });
}
