//! Icon source selection and synchronous cache lookup

use gtk::prelude::*;
use unixnotis_core::NotificationView;

use super::cache::{icon_key_for_name, icon_key_for_path, set_image_key, CachedPaintable};
use super::resolver::IconResolverInner;
use super::theme::{
    collect_icon_candidates, image_data_texture, image_data_texture_for_data, resolve_icon_source,
    IconSource,
};
use super::types::{IconDecodeRequest, IconResolution};

impl IconResolverInner {
    pub(super) fn apply_sender_visual(&self, image: &gtk::Image, notification: &NotificationView) {
        // The daemon has already decoded and bounded this sender-provided raster
        if !matches!(
            notification.image.sender_visual_role,
            unixnotis_core::NotificationVisualRole::ConversationAvatar
                | unixnotis_core::NotificationVisualRole::ApplicationProvidedIcon
        ) {
            image.set_visible(false);
            return;
        }
        if let Some(texture) = image_data_texture_for_data(&notification.image.sender_visual) {
            image.set_paintable(Some(&texture));
            image.set_visible(true);
            return;
        }
        image.set_visible(false);
    }

    pub(super) fn apply_content_visual(&self, image: &gtk::Image, notification: &NotificationView) {
        // Content pixels were bounded by the daemon before reaching GTK
        if let Some(texture) = image_data_texture(&notification.image) {
            image.set_paintable(Some(&texture));
            image.set_visible(true);
        } else {
            image.set_visible(false);
        }
    }

    pub(super) fn apply_badge(
        &self,
        image: &gtk::Image,
        notification: &NotificationView,
        size: i32,
        scale: i32,
    ) {
        if let Some(resolved) = self.resolve_badge(notification, size, scale) {
            self.apply_resolution(image, resolved);
            return;
        }
        image.set_visible(false);
    }

    pub(super) fn clear_missing_cache(&self) {
        self.missing_names.borrow_mut().clear();
    }

    fn resolve_badge(
        &self,
        notification: &NotificationView,
        size: i32,
        scale: i32,
    ) -> Option<IconResolution> {
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

    fn apply_resolution(&self, image: &gtk::Image, resolved: IconResolution) {
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
                    },
                })
            }
        }
    }
}

const fn icon_name_is_usable(name: &str) -> bool {
    !name.is_empty()
}

#[cfg(test)]
#[path = "tests/resolution.rs"]
mod tests;
