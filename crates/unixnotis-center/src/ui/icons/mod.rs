//! Icon resolution for notification widgets.
//!
//! Keeps icon orchestration in this module while delegating cache and
//! decoding helpers to focused submodules.

mod icons_cache;
mod icons_decode;
mod icons_sources;

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::Path;
use std::rc::Rc;

use gtk::glib;
use gtk::prelude::*;
use tracing::debug;
use unixnotis_core::NotificationView;

use icons_cache::{
    icon_key_for_image, icon_key_for_name, icon_key_for_path, image_key_matches, set_image_key,
    CachedPaintable, IconCache, IconKey,
};
use icons_decode::{texture_from_raster, IconResult, IconUpdate, IconWorker};
use icons_sources::{
    collect_icon_candidates, file_path_from_hint, image_data_texture, is_svg_path,
    resolve_icon_source, resolve_path_texture, DesktopIconIndex, IconSource,
};

/// Resolves notification icons using image hints, themed icons, and desktop metadata.
pub struct IconResolver {
    inner: Rc<IconResolverInner>,
}

impl IconResolver {
    pub fn new() -> Self {
        let (update_tx, update_rx) = async_channel::unbounded::<IconUpdate>();
        let worker = IconWorker::new(update_tx);
        let inner = Rc::new(IconResolverInner {
            desktop_index: DesktopIconIndex::new(),
            cache: RefCell::new(IconCache::new(256)),
            inflight: RefCell::new(HashMap::new()),
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
}

struct IconResolverInner {
    desktop_index: DesktopIconIndex,
    cache: RefCell<IconCache>,
    inflight: RefCell<HashMap<IconKey, Vec<glib::WeakRef<gtk::Image>>>>,
    worker: IconWorker,
}

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
                IconResolution::Async { key, request } => {
                    set_image_key(image, key.clone());
                    self.enqueue(request, image);
                    image.set_visible(false);
                }
            }
            return;
        }

        image.set_visible(false);
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
                let path_buf = path.to_path_buf();
                if let Some(key) = icon_key_for_path(path, size, scale) {
                    if let Some(paintable) = self.cache.borrow_mut().get(&key) {
                        return Some(IconResolution::Ready { key, paintable });
                    }
                    if is_svg_path(path) {
                        if let Some(paintable) = resolve_path_texture(path) {
                            let paintable = self.cache.borrow_mut().insert(key.clone(), paintable);
                            return Some(IconResolution::Ready { key, paintable });
                        }
                        return None;
                    }
                    return Some(IconResolution::Async {
                        key: key.clone(),
                        request: IconDecodeRequest {
                            key,
                            path: path_buf,
                            size,
                            scale,
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
        if let Some(key) = icon_key_for_name(name, size, scale) {
            if let Some(cached) = self.cache.borrow_mut().get(&key) {
                return Some(IconResolution::Ready {
                    key,
                    paintable: cached,
                });
            }
        }
        let source = resolve_icon_source(name, size, scale)?;
        match source {
            IconSource::Paintable(paintable) => {
                let key = icon_key_for_name(name, size, scale)?;
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
                    key: key.clone(),
                    request: IconDecodeRequest {
                        key,
                        path,
                        size,
                        scale,
                    },
                })
            }
        }
    }

    fn enqueue(&self, request: IconDecodeRequest, image: &gtk::Image) {
        let mut inflight = self.inflight.borrow_mut();
        if let Some(waiters) = inflight.get_mut(&request.key) {
            waiters.push(image.downgrade());
            return;
        }
        inflight.insert(request.key.clone(), vec![image.downgrade()]);
        self.worker.submit_decode(
            request.key.clone(),
            request.path.clone(),
            request.size,
            request.scale,
        );
    }

    fn handle_update(&self, update: IconUpdate) {
        let waiters = self
            .inflight
            .borrow_mut()
            .remove(&update.key)
            .unwrap_or_default();

        let paintable = match update.result {
            IconResult::Raster(image) => {
                let texture = texture_from_raster(&image);
                Some(
                    self.cache
                        .borrow_mut()
                        .insert(update.key.clone(), CachedPaintable::from_texture(texture)),
                )
            }
            IconResult::Failed(err) => {
                debug!(?err, "icon decode failed");
                match &update.key {
                    IconKey::Path { path, .. } => resolve_path_texture(Path::new(path))
                        .map(|texture| self.cache.borrow_mut().insert(update.key.clone(), texture)),
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
            if image_key_matches(&image, &update.key) {
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

enum IconResolution {
    Ready {
        key: IconKey,
        paintable: Rc<CachedPaintable>,
    },
    Async {
        key: IconKey,
        request: IconDecodeRequest,
    },
}

struct IconDecodeRequest {
    key: IconKey,
    path: std::path::PathBuf,
    size: i32,
    scale: i32,
}
