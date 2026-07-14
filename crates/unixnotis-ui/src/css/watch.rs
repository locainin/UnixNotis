//! File watcher helpers for CSS and config hot reload

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher};
use tracing::warn;
use unixnotis_core::ThemePaths;

use super::CssKind;

/// Start a file watcher for CSS paths and emit reload callbacks
/// Start a watcher for the configured CSS directories
///
/// # Errors
///
/// Returns an error when the watcher cannot be created or registered
pub fn start_css_watcher(
    paths: &ThemePaths,
    kind: CssKind,
    on_reload: impl Fn() + Send + 'static,
) -> notify::Result<()> {
    let mut watched_dirs = HashSet::new();
    let css_paths = match kind {
        CssKind::Panel => vec![
            &paths.base_css,
            &paths.panel_css,
            &paths.widgets_css,
            &paths.media_css,
        ],
        CssKind::Popup => vec![&paths.base_css, &paths.popup_css],
    };
    for path in css_paths {
        if let Some(dir) = path.parent() {
            watched_dirs.insert(dir.to_path_buf());
        }
    }

    if watched_dirs.is_empty() {
        return Ok(());
    }

    let (event_tx, event_rx) = mpsc::channel::<notify::Result<Event>>();
    let mut watcher = RecommendedWatcher::new(
        move |res| {
            let _ = event_tx.send(res);
        },
        notify::Config::default(),
    )?;
    for dir in &watched_dirs {
        watcher.watch(dir, RecursiveMode::NonRecursive)?;
    }

    thread::spawn(move || {
        // Moving the watcher into the worker keeps every registration alive
        let _watcher = watcher;
        let debounce = Duration::from_millis(150);
        // Block on recv so the watcher thread does not wake periodically when idle
        // Using recv_timeout here would wake every debounce interval and burn CPU
        // even when no files change
        while let Ok(event) = event_rx.recv() {
            if let Err(err) = event {
                warn!(?err, "css watcher reported an error");
                continue;
            }
            // Once an event arrives, coalesce bursts by waiting for a quiet window
            // This keeps reloads responsive while minimizing redundant reload work
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
    Ok(())
}

/// Start a file watcher for the config path and emit reload callbacks
/// Start a watcher for the active configuration file
///
/// # Errors
///
/// Returns an error when the watcher cannot be created or registered
pub fn start_config_watcher(
    config_path: &Path,
    on_reload: impl Fn() + Send + 'static,
) -> notify::Result<()> {
    let Some(parent) = config_path.parent().map(PathBuf::from) else {
        return Ok(());
    };
    let config_name = config_path.file_name().map(std::ffi::OsStr::to_os_string);
    let (event_tx, event_rx) = mpsc::channel::<notify::Result<Event>>();
    let mut watcher = RecommendedWatcher::new(
        move |res| {
            let _ = event_tx.send(res);
        },
        notify::Config::default(),
    )?;
    watcher.watch(&parent, RecursiveMode::NonRecursive)?;

    thread::spawn(move || {
        // Moving the watcher into the worker keeps the directory watch alive
        let _watcher = watcher;
        let debounce = Duration::from_millis(150);
        // Block on recv so the watcher thread does not wake periodically when idle
        // Using recv_timeout here would wake every debounce interval and burn CPU
        // even when no files change
        while let Ok(event) = event_rx.recv() {
            let Ok(event) = event else {
                warn!(?event, "config watcher reported an error");
                continue;
            };
            if !event_targets_config(&event, config_name.as_deref()) {
                continue;
            }
            // Coalesce rapid edits by draining events until the debounce window is quiet
            // This avoids multiple reloads during a single save operation
            loop {
                match event_rx.recv_timeout(debounce) {
                    Ok(event) => {
                        if let Err(error) = event {
                            warn!(?error, "config watcher reported an error");
                        }
                        // Every nearby event extends the quiet window so atomic editor renames
                        // cannot trigger a reload before the final config path settles
                    }
                    Err(mpsc::RecvTimeoutError::Timeout) => break,
                    Err(mpsc::RecvTimeoutError::Disconnected) => return,
                }
            }
            on_reload();
        }
    });
    Ok(())
}

fn event_targets_config(event: &Event, config_name: Option<&std::ffi::OsStr>) -> bool {
    config_name.is_none_or(|name| {
        event
            .paths
            .iter()
            .any(|path| path.file_name() == Some(name))
    })
}

#[cfg(test)]
#[path = "tests/watch.rs"]
mod tests;
