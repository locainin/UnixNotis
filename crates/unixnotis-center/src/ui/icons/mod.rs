//! Icon resolution for notification widgets.
//!
//! Keeps icon orchestration in this module while delegating cache and
//! decoding helpers to focused submodules.

mod cache;
mod decode;
mod index;
mod theme;

use std::cell::RefCell;
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::Path;
use std::rc::Rc;
use std::time::{Duration, Instant};

use gdk_pixbuf::Pixbuf;
use gio::MemoryInputStream;
use gtk::gdk;
use gtk::glib;
use gtk::prelude::*;
use tracing::debug;
use unixnotis_core::NotificationView;

use cache::{
    icon_key_for_image, icon_key_for_name, icon_key_for_path, image_key_matches, set_image_key,
    CachedPaintable, IconCache, IconKey,
};
use decode::{texture_from_raster, IconDecodeMode, IconResult, IconUpdate, IconWorker};
use index::DesktopIconIndex;
use theme::{
    collect_icon_candidates, file_path_from_hint, image_data_texture, is_svg_path,
    resolve_icon_source, resolve_path_texture, IconSource,
};

/// Resolves notification icons using image hints, themed icons, and desktop metadata.
pub struct IconResolver {
    inner: Rc<IconResolverInner>,
}

impl IconResolver {
    pub fn new() -> Self {
        // Bound update queue to avoid unbounded memory growth if UI stalls.
        let (update_tx, update_rx) =
            async_channel::bounded::<IconUpdate>(ICON_UPDATE_QUEUE_CAPACITY);
        let worker = IconWorker::new(update_tx);
        let inner = Rc::new(IconResolverInner {
            desktop_index: DesktopIconIndex::new(),
            cache: RefCell::new(IconCache::new(256)),
            inflight: RefCell::new(HashMap::new()),
            missing_names: RefCell::new(MissingIconCache::new(512)),
            worker,
        });
        let inner_clone = inner.clone();
        glib::MainContext::default().spawn_local(async move {
            while let Ok(update) = update_rx.recv().await {
                inner_clone.handle_update(update);
            }
        });

        Self { inner }
    }

    pub fn apply_icon(
        &self,
        image: &gtk::Image,
        notification: &NotificationView,
        size: i32,
        scale: i32,
    ) {
        self.inner.apply_icon(image, notification, size, scale);
    }

    pub fn clear_missing_cache(&self) {
        // Theme reloads can add icons that were previously missing.
        // Clearing the miss cache ensures new lookups are attempted immediately
        // instead of waiting for the miss TTL to expire.
        self.inner.clear_missing_cache();
    }
}

struct IconResolverInner {
    desktop_index: DesktopIconIndex,
    cache: RefCell<IconCache>,
    inflight: RefCell<HashMap<IconKey, Vec<glib::WeakRef<gtk::Image>>>>,
    missing_names: RefCell<MissingIconCache>,
    worker: IconWorker,
}

// Update queue capacity keeps bursts buffered without unbounded growth.
const ICON_UPDATE_QUEUE_CAPACITY: usize = 256;

impl IconResolverInner {
    fn apply_icon(
        &self,
        image: &gtk::Image,
        notification: &NotificationView,
        size: i32,
        scale: i32,
    ) {
        if let Some(resolved) = self.resolve_icon(notification, size, scale) {
            match resolved {
                IconResolution::Ready { key, paintable } => {
                    set_image_key(image, key);
                    image.set_paintable(Some(paintable.paintable()));
                    image.set_visible(true);
                }
                IconResolution::Async { request } => {
                    // The async request already owns the cache key, so the widget
                    // only needs one cheap clone before the request moves away
                    set_image_key(image, request.key.clone());
                    self.enqueue(request, image);
                    image.set_visible(false);
                }
            }
            return;
        }

        image.set_visible(false);
    }

    fn clear_missing_cache(&self) {
        // Clear both ordered and set storage to keep cache state consistent.
        self.missing_names.borrow_mut().clear();
    }

    fn resolve_icon(
        &self,
        notification: &NotificationView,
        size: i32,
        scale: i32,
    ) -> Option<IconResolution> {
        let image = &notification.image;
        if let Some(key) = icon_key_for_image(image, size, scale) {
            if let Some(paintable) = self.lookup_cached(key.clone(), || {
                image_data_texture(image).map(CachedPaintable::from_texture)
            }) {
                return Some(IconResolution::Ready { key, paintable });
            }
        }

        if !image.image_path.is_empty() {
            if let Some(path) = file_path_from_hint(&image.image_path) {
                // Own the decoded path to keep icon decode jobs self-contained.
                if let Some(key) = icon_key_for_path(&path, size, scale) {
                    if let Some(paintable) = self.cache.borrow_mut().get(&key) {
                        return Some(IconResolution::Ready { key, paintable });
                    }
                    if is_svg_path(&path) {
                        // SVG icon hints should avoid synchronous file I/O on the GTK thread.
                        // Route them through the async byte loader so texture creation happens on
                        // the main loop without blocking disk reads.
                        return Some(IconResolution::Async {
                            request: IconDecodeRequest {
                                key,
                                path,
                                size,
                                scale,
                                mode: IconDecodeMode::Bytes,
                            },
                        });
                    }
                    return Some(IconResolution::Async {
                        request: IconDecodeRequest {
                            key,
                            path,
                            size,
                            scale,
                            mode: IconDecodeMode::Raster,
                        },
                    });
                }
            }
        }

        if let Some(resolution) = self.resolve_icon_name(image.icon_name.as_str(), size, scale) {
            return Some(resolution);
        }

        let candidates = collect_icon_candidates(notification);
        for candidate in &candidates {
            if let Some(icons) = self.desktop_index.icons_for(candidate) {
                for icon_name in icons {
                    if let Some(resolution) =
                        self.resolve_icon_name(icon_name.as_str(), size, scale)
                    {
                        return Some(resolution);
                    }
                }
            }
        }

        for candidate in candidates {
            if let Some(resolution) = self.resolve_icon_name(candidate.as_str(), size, scale) {
                return Some(resolution);
            }
        }

        None
    }

    fn resolve_icon_name(&self, name: &str, size: i32, scale: i32) -> Option<IconResolution> {
        if name.is_empty() {
            return None;
        }
        let key = icon_key_for_name(name, size, scale)?;
        if self.missing_names.borrow_mut().contains(&key) {
            return None;
        }
        if let Some(cached) = self.cache.borrow_mut().get(&key) {
            return Some(IconResolution::Ready {
                key,
                paintable: cached,
            });
        }
        let source = match resolve_icon_source(name, size, scale) {
            Some(source) => source,
            None => {
                // Cache misses briefly to avoid repeated theme lookups during bursts.
                self.missing_names.borrow_mut().insert(key.clone());
                return None;
            }
        };
        match source {
            IconSource::Paintable(paintable) => {
                if let Some(cached) = self.cache.borrow_mut().get(&key) {
                    return Some(IconResolution::Ready {
                        key,
                        paintable: cached,
                    });
                }
                let cached = self
                    .cache
                    .borrow_mut()
                    .insert(key.clone(), CachedPaintable::from_icon(paintable));
                Some(IconResolution::Ready {
                    key,
                    paintable: cached,
                })
            }
            IconSource::RasterPath(path) => {
                let key = icon_key_for_path(path.as_path(), size, scale)?;
                if let Some(paintable) = self.cache.borrow_mut().get(&key) {
                    return Some(IconResolution::Ready { key, paintable });
                }
                Some(IconResolution::Async {
                    request: IconDecodeRequest {
                        key,
                        path,
                        size,
                        scale,
                        mode: IconDecodeMode::Raster,
                    },
                })
            }
        }
    }

    fn enqueue(&self, request: IconDecodeRequest, image: &gtk::Image) {
        // Split the request here so the owned pieces can move into the worker
        // instead of cloning the same path and key again on the hot path
        let IconDecodeRequest {
            key,
            path,
            size,
            scale,
            mode,
        } = request;
        let mut inflight = self.inflight.borrow_mut();
        if let Some(waiters) = inflight.get_mut(&key) {
            waiters.push(image.downgrade());
            return;
        }
        inflight.insert(key.clone(), vec![image.downgrade()]);
        // Release the inflight borrow before handling synchronous errors.
        drop(inflight);
        // Keep one local key around so a failed submit can clear the same inflight
        // entry without rebuilding a second owned key from raw inputs
        if let Err(err) = self
            .worker
            .submit_decode(key.clone(), path, size, scale, mode)
        {
            // Keep the inflight map consistent by issuing an immediate failure update.
            self.handle_update(IconUpdate {
                key,
                result: IconResult::Failed(err.reason().to_string()),
            });
        }
    }

    fn handle_update(&self, update: IconUpdate) {
        let IconUpdate { key, result } = update;
        let waiters = self.inflight.borrow_mut().remove(&key).unwrap_or_default();

        let paintable = match result {
            IconResult::Raster(image) => {
                let texture = texture_from_raster(&image);
                Some(
                    self.cache
                        .borrow_mut()
                        .insert(key.clone(), CachedPaintable::from_texture(texture)),
                )
            }
            IconResult::Bytes(bytes) => {
                // Create textures on the GTK thread to keep GDK/GIO object use thread-safe.
                // The bytes were loaded off-thread to avoid synchronous disk I/O here.
                let (size, scale) = key.size_and_scale();
                let target = size.max(1).saturating_mul(scale.max(1)).max(1);
                // Decode at the target size to avoid large SVG rasterization on the UI thread.
                let bytes = glib::Bytes::from_owned(bytes);
                let stream = MemoryInputStream::from_bytes(&bytes);
                match Pixbuf::from_stream_at_scale(
                    &stream,
                    target,
                    target,
                    true,
                    None::<&gio::Cancellable>,
                ) {
                    Ok(pixbuf) => {
                        let texture = gdk::Texture::for_pixbuf(&pixbuf);
                        Some(
                            self.cache
                                .borrow_mut()
                                .insert(key.clone(), CachedPaintable::from_texture(texture)),
                        )
                    }
                    Err(err) => {
                        debug!(?err, "icon byte decode failed");
                        None
                    }
                }
            }
            IconResult::Failed(err) => {
                debug!(?err, "icon decode failed");
                match &key {
                    IconKey::Path { path, .. } => resolve_path_texture(Path::new(path))
                        .map(|texture| self.cache.borrow_mut().insert(key.clone(), texture)),
                    _ => None,
                }
            }
        };

        let Some(paintable) = paintable else {
            return;
        };
        for waiter in waiters {
            let Some(image) = waiter.upgrade() else {
                continue;
            };
            if image_key_matches(&image, &key) {
                image.set_paintable(Some(paintable.paintable()));
                image.set_visible(true);
            }
        }
    }

    fn lookup_cached<F>(&self, key: IconKey, build: F) -> Option<Rc<CachedPaintable>>
    where
        F: FnOnce() -> Option<CachedPaintable>,
    {
        if let Some(paintable) = self.cache.borrow_mut().get(&key) {
            return Some(paintable);
        }
        let paintable = build()?;
        Some(self.cache.borrow_mut().insert(key, paintable))
    }
}

// Cache failed icon lookups briefly to avoid repeated theme scans during bursts.
// Entries expire quickly to avoid pinning misses after icon theme changes.
struct MissingIconCache {
    order: VecDeque<(IconKey, Instant)>,
    set: HashSet<IconKey>,
    max_entries: usize,
}

impl MissingIconCache {
    fn new(max_entries: usize) -> Self {
        Self {
            order: VecDeque::new(),
            set: HashSet::new(),
            max_entries,
        }
    }

    fn contains(&mut self, key: &IconKey) -> bool {
        self.purge_expired();
        self.set.contains(key)
    }

    fn insert(&mut self, key: IconKey) {
        self.purge_expired();
        if !self.set.insert(key.clone()) {
            return;
        }
        self.order.push_back((key, Instant::now()));
        while self.order.len() > self.max_entries {
            if let Some((evicted, _)) = self.order.pop_front() {
                self.set.remove(&evicted);
            }
        }
    }

    fn purge_expired(&mut self) {
        let ttl = Duration::from_secs(30);
        let now = Instant::now();
        while let Some((_, timestamp)) = self.order.front() {
            if now.duration_since(*timestamp) < ttl {
                break;
            }
            // Pop the expired key out of the queue first so the owned key can move
            // straight into the set removal without another clone
            let Some((key, _)) = self.order.pop_front() else {
                break;
            };
            self.set.remove(&key);
        }
    }

    fn clear(&mut self) {
        // Clear both the ordered list and set to reset miss tracking.
        // This avoids reusing stale misses after theme changes.
        self.order.clear();
        self.set.clear();
    }
}

enum IconResolution {
    Ready {
        key: IconKey,
        paintable: Rc<CachedPaintable>,
    },
    Async {
        request: IconDecodeRequest,
    },
}

struct IconDecodeRequest {
    key: IconKey,
    path: std::path::PathBuf,
    size: i32,
    scale: i32,
    mode: IconDecodeMode,
}
