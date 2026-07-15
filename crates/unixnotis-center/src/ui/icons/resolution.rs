//! Icon source selection and synchronous cache lookup

use std::rc::Rc;

use gtk::prelude::*;
use unixnotis_core::NotificationView;

use super::cache::{
    icon_key_for_image, icon_key_for_name, icon_key_for_path, set_image_key, CachedPaintable,
    IconKey,
};
use super::decode::IconDecodeMode;
use super::resolver::IconResolverInner;
use super::theme::{
    collect_icon_candidates, file_path_from_hint, image_data_texture, is_svg_path,
    resolve_icon_source, IconSource,
};
use super::types::{IconDecodeRequest, IconResolution};

impl IconResolverInner {
    pub(super) fn apply_icon(
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
                    set_image_key(image, request.key.clone());
                    self.enqueue(request, image);
                    image.set_visible(false);
                }
            }
            return;
        }

        image.set_visible(false);
    }

    pub(super) fn clear_missing_cache(&self) {
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

        if let Some(path) = file_path_from_hint(&image.image_path) {
            // File paths use asynchronous decoding so disk I/O stays off GTK
            if let Some(key) = icon_key_for_path(&path, size, scale) {
                if let Some(paintable) = self.cache.borrow_mut().get(&key) {
                    return Some(IconResolution::Ready { key, paintable });
                }
                let mode = if is_svg_path(&path) {
                    IconDecodeMode::Bytes
                } else {
                    IconDecodeMode::Raster
                };
                return Some(IconResolution::Async {
                    request: IconDecodeRequest {
                        key,
                        path,
                        size,
                        scale,
                        mode,
                    },
                });
            }
        }

        if let Some(resolution) = self.resolve_icon_name(&image.icon_name, size, scale) {
            return Some(resolution);
        }

        let candidates = collect_icon_candidates(notification);
        for candidate in &candidates {
            if let Some(icons) = self.desktop_index.icons_for(candidate) {
                for icon_name in icons {
                    if let Some(resolution) = self.resolve_icon_name(&icon_name, size, scale) {
                        return Some(resolution);
                    }
                }
            }
        }
        for candidate in candidates {
            if let Some(resolution) = self.resolve_icon_name(&candidate, size, scale) {
                return Some(resolution);
            }
        }
        None
    }

    fn resolve_icon_name(&self, name: &str, size: i32, scale: i32) -> Option<IconResolution> {
        if !icon_name_is_usable(name) {
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
        let Some(source) = resolve_icon_source(name, size, scale) else {
            // Brief negative caching keeps repeated theme scans out of bursts
            self.missing_names.borrow_mut().insert(key);
            return None;
        };
        match source {
            IconSource::Paintable(paintable) => {
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
                let key = icon_key_for_path(&path, size, scale)?;
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

const fn icon_name_is_usable(name: &str) -> bool {
    !name.is_empty()
}

#[cfg(test)]
#[path = "tests/resolution.rs"]
mod tests;
