//! Debounced desktop-index refresh with atomic snapshot replacement

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use arc_swap::ArcSwap;
use notify::{RecursiveMode, Watcher};
use tokio::sync::mpsc;
use tracing::{debug, warn};

use super::model::DesktopIdentityIndex;
use super::scan::desktop_roots;

const REFRESH_DEBOUNCE: Duration = Duration::from_millis(500);
const REFRESH_SIGNAL_CAPACITY: usize = 1;

pub fn spawn_desktop_index_refresh(
    index: Arc<ArcSwap<DesktopIdentityIndex>>,
) -> Result<tokio::task::JoinHandle<()>> {
    let (refresh_tx, mut refresh_rx) = mpsc::channel(REFRESH_SIGNAL_CAPACITY);
    let mut watcher = notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
        match event {
            Ok(_) => {
                // A single pending signal coalesces filesystem bursts without blocking the watcher
                let _ = refresh_tx.try_send(());
            }
            Err(error) => warn!(?error, "desktop application watcher reported an error"),
        }
    })
    .context("create desktop application watcher")?;

    let mut watched_root = false;
    for (root, _) in desktop_roots() {
        if !root.is_dir() {
            continue;
        }
        match watcher.watch(Path::new(&root), RecursiveMode::Recursive) {
            Ok(()) => watched_root = true,
            Err(error) => warn!(
                ?error,
                root = %root.display(),
                "failed to watch desktop application directory"
            ),
        }
    }
    if !watched_root {
        warn!("no desktop application directory is available for refresh watching");
    }

    Ok(tokio::spawn(async move {
        // The watcher must stay owned by this task for kernel watches to remain registered
        let _watcher = watcher;
        while refresh_rx.recv().await.is_some() {
            tokio::time::sleep(REFRESH_DEBOUNCE).await;
            // Drain events that arrived during the debounce window before one complete rebuild
            while refresh_rx.try_recv().is_ok() {}
            match tokio::task::spawn_blocking(DesktopIdentityIndex::new).await {
                Ok(rebuilt) => {
                    index.store(Arc::new(rebuilt));
                    debug!("desktop application identity index refreshed");
                }
                Err(error) => {
                    warn!(?error, "desktop application identity index rebuild failed");
                }
            }
        }
    }))
}
