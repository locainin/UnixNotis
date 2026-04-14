//! Icon decode, cache, and widget construction for popups.
//!
//! Keeps icon decoding, caching, and texture reuse isolated from UI state handling.

use std::path::PathBuf;

use gtk::glib::object::Cast;
use gtk::prelude::*;
use gtk::{gdk, glib};
use tracing::debug;
use unixnotis_core::NotificationView;

use super::icons::{
    collect_icon_candidates, file_path_from_hint, image_data_texture, resolve_icon_image,
    IconDecodePool, IconDecodeResult,
};
use super::UiState;

const ICON_CACHE_MAX_ENTRIES: usize = 256;
// Skip caching decoded textures above this size to avoid holding large buffers.
const ICON_TEXTURE_CACHE_MAX_BYTES: usize = 1024 * 1024;
// Popup icon size is fixed so rows stay visually consistent across icon sources.
const POPUP_ICON_SIZE: i32 = 20;

impl UiState {
    pub(super) fn build_image_widget(
        &mut self,
        notification: &NotificationView,
    ) -> Option<gtk::Image> {
        let image = &notification.image;
        if let Some(texture) = image_data_texture(image) {
            let widget = gtk::Image::from_paintable(Some(&texture));
            set_popup_icon_size(&widget, POPUP_ICON_SIZE);
            return Some(widget);
        }

        if !image.image_path.is_empty() {
            let path = image.image_path.as_str();
            return self.resolve_icon_widget(path, POPUP_ICON_SIZE);
        }

        let cache_key = format!("{}|{}", notification.app_name, notification.image.icon_name);
        if let Some(cached) = self.icon_cache.get(&cache_key) {
            return cached
                .as_ref()
                .and_then(|icon_name| self.resolve_icon_widget(icon_name, POPUP_ICON_SIZE));
        }

        let candidates = collect_icon_candidates(notification);
        // Keep the first successful resolve to avoid duplicate theme lookups and widget creation.
        let mut resolved: Option<(String, gtk::Image)> = None;

        for candidate in &candidates {
            if let Some(icon_names) = self.desktop_icons.icons_for(candidate) {
                for icon_name in icon_names {
                    if let Some(widget) =
                        self.resolve_icon_widget(icon_name.as_str(), POPUP_ICON_SIZE)
                    {
                        resolved = Some((icon_name.clone(), widget));
                        break;
                    }
                }
                if resolved.is_some() {
                    break;
                }
            }
        }

        if resolved.is_none() {
            for candidate in &candidates {
                if let Some(widget) = self.resolve_icon_widget(candidate, POPUP_ICON_SIZE) {
                    resolved = Some((candidate.clone(), widget));
                    break;
                }
            }
        }

        match resolved {
            Some((icon_name, widget)) => {
                self.cache_icon(cache_key, Some(icon_name));
                Some(widget)
            }
            None => {
                self.cache_icon(cache_key, None);
                None
            }
        }
    }

    fn cache_icon(&mut self, cache_key: String, resolved: Option<String>) {
        // Bound the icon cache to avoid unbounded growth in long-running sessions.
        match self.icon_cache.entry(cache_key) {
            std::collections::hash_map::Entry::Occupied(mut entry) => {
                entry.insert(resolved);
                return;
            }
            std::collections::hash_map::Entry::Vacant(entry) => {
                let key = entry.key().clone();
                entry.insert(resolved);
                self.icon_cache_order.push_back(key);
            }
        }
        while self.icon_cache_order.len() > ICON_CACHE_MAX_ENTRIES {
            if let Some(evicted) = self.icon_cache_order.pop_front() {
                self.icon_cache.remove(&evicted);
            }
        }
    }

    fn resolve_icon_widget(&self, name: &str, size: i32) -> Option<gtk::Image> {
        if let Some(file_path) = file_path_from_hint(name) {
            // Decoded file:// paths allow loading icon files with escaped characters.
            if file_path.is_file() {
                // Reuse a cached texture when available to avoid repeated decode work.
                if let Some(texture) = self.icon_texture_cache.borrow_mut().get(&file_path) {
                    let widget = gtk::Image::new();
                    widget.set_paintable(Some(&texture));
                    set_popup_icon_size(&widget, size);
                    return Some(widget);
                }
                return Some(self.spawn_file_icon(file_path, size));
            }
        }
        let widget = resolve_icon_image(name, size)?;
        set_popup_icon_size(&widget, size);
        Some(widget)
    }

    fn spawn_file_icon(&self, path: PathBuf, size: i32) -> gtk::Image {
        let widget = gtk::Image::new();
        set_popup_icon_size(&widget, size);
        let (tx, rx) = async_channel::bounded::<IconDecodeResult>(1);
        let widget_clone = widget.clone();
        let cache = self.icon_texture_cache.clone();
        let path_clone = path.clone();
        let target_size = size.max(1);
        // Apply the texture on the main loop to avoid GTK thread violations.
        glib::MainContext::default().spawn_local(async move {
            if let Ok(result) = rx.recv().await {
                match result {
                    Ok(icon) => {
                        let bytes = glib::Bytes::from(&icon.bytes);
                        let texture = gdk::MemoryTexture::new(
                            icon.width,
                            icon.height,
                            gdk::MemoryFormat::R8g8b8a8,
                            &bytes,
                            icon.stride as usize,
                        )
                        .upcast::<gdk::Texture>();
                        // Cache only modestly sized textures to limit resident memory.
                        if icon.bytes.len() <= ICON_TEXTURE_CACHE_MAX_BYTES {
                            cache
                                .borrow_mut()
                                .insert(path_clone.clone(), texture.clone());
                        }
                        widget_clone.set_paintable(Some(&texture));
                        set_popup_icon_size(&widget_clone, target_size);
                    }
                    Err(err) => {
                        debug!(?err, "popup icon decode failed");
                    }
                }
            }
        });

        // Decode on a background worker pool to avoid spawning unbounded threads.
        IconDecodePool::global().submit(path, target_size, tx);

        widget
    }
}

fn set_popup_icon_size(widget: &gtk::Image, size: i32) {
    let size = size.max(1);
    // Enforce a fixed icon footprint so file-backed and themed icons align.
    widget.set_pixel_size(size);
    widget.set_size_request(size, size);
}
