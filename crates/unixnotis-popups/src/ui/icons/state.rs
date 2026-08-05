//! Icon decode, cache, and widget construction for popups
//!
//! Keeps icon decoding, caching, and texture reuse isolated from UI state handling

use std::path::PathBuf;
use std::rc::Rc;
use std::time::{Duration, Instant};

use gtk::glib::object::Cast;
use gtk::prelude::*;
use gtk::{gdk, glib};
use tracing::debug;
use unixnotis_core::NotificationView;

use super::super::state::{IconCacheEntry, IconResolutionKey};
use super::super::UiState;
use super::{
    collect_icon_candidates, file_path_from_hint, image_data_texture, image_data_texture_for_data,
    IconDecodePool, IconDecodeResult, TextureCache, ThemeIconCache,
};

const ICON_CACHE_MAX_ENTRIES: usize = 256;
// Skip caching decoded textures above this size to avoid holding large buffers
const ICON_TEXTURE_CACHE_MAX_BYTES: usize = 1_048_576;
// Content stays visibly separate from the daemon-associated application badge
const POPUP_CONTENT_THUMBNAIL_SIZE: i32 = 64;
// Decorative sender art is a small context cue, not the notification identity
const POPUP_APPLICATION_VISUAL_SIZE: i32 = 38;
// Missing icons are retried soon so package and theme installs heal without a process restart
const NEGATIVE_ICON_CACHE_TTL: Duration = Duration::from_secs(15);

impl UiState {
    pub(in crate::ui) fn build_conversation_avatar_widget(
        notification: &NotificationView,
        size: i32,
    ) -> Option<gtk::Image> {
        // Conversation art is safe to render here because the daemon sent pixels, not a path
        if !matches!(
            notification.image.sender_visual_role,
            unixnotis_core::NotificationVisualRole::ConversationAvatar
        ) {
            return None;
        }

        let texture = image_data_texture_for_data(&notification.image.sender_visual)?;
        let widget = gtk::Image::from_paintable(Some(&texture));
        set_popup_icon_size(&widget, size);
        widget.add_css_class("unixnotis-popup-conversation-avatar");
        Some(widget)
    }

    pub(in crate::ui) fn build_sender_visual_widget(
        notification: &NotificationView,
    ) -> Option<gtk::Image> {
        if notification.image.sender_visual_role
            != unixnotis_core::NotificationVisualRole::ApplicationProvidedIcon
        {
            return None;
        }
        let texture = image_data_texture_for_data(&notification.image.sender_visual)?;
        let widget = gtk::Image::from_paintable(Some(&texture));
        set_popup_icon_size(&widget, POPUP_APPLICATION_VISUAL_SIZE);
        widget.add_css_class("unixnotis-popup-application-visual");
        Some(widget)
    }

    pub(in crate::ui) fn build_content_image_widget(
        notification: &NotificationView,
    ) -> Option<gtk::Image> {
        if let Some(texture) = image_data_texture(&notification.image) {
            let widget = gtk::Image::from_paintable(Some(&texture));
            set_popup_icon_size(&widget, POPUP_CONTENT_THUMBNAIL_SIZE);
            return Some(widget);
        }

        None
    }

    pub(in crate::ui) fn build_app_icon_widget(
        &mut self,
        notification: &NotificationView,
        size: i32,
    ) -> Option<gtk::Image> {
        self.refresh_icon_sources_if_needed();
        // Caller image hints are content, so the header resolves only authenticated badge inputs
        let cache_key = IconResolutionKey {
            app_name: notification.app_name.clone(),
            badge_icon: notification.attribution.badge_icon.clone(),
            desktop_id: notification.attribution.desktop_id.clone(),
            claimed_theme_icon: notification.image.claimed_theme_icon.clone(),
        };
        if let Some(cached) = self.icon_cache.get(&cache_key) {
            if let Some(icon_name) = cached.resolved.as_deref() {
                return resolve_icon_widget(
                    &mut self.theme_icon_cache,
                    &self.icon_texture_cache,
                    icon_name,
                    size,
                );
            }
            if negative_cache_is_fresh(cached.cached_at, Instant::now()) {
                return None;
            }
            // Expired misses fall through to a real desktop and icon-theme lookup
            self.icon_cache.remove(&cache_key);
            self.icon_cache_order.retain(|key| key != &cache_key);
        }

        let candidates = collect_icon_candidates(notification);
        // Keep the first successful resolve to avoid duplicate theme lookups and widget creation
        let mut resolved: Option<(String, gtk::Image)> = None;

        for candidate in &candidates {
            if let Some(icon_names) = self.desktop_icons.icons_for(candidate) {
                for icon_name in icon_names {
                    if let Some(widget) = resolve_icon_widget(
                        &mut self.theme_icon_cache,
                        &self.icon_texture_cache,
                        icon_name.as_str(),
                        size,
                    ) {
                        resolved = Some((icon_name, widget));
                        break;
                    }
                }
                if resolved.is_some() {
                    break;
                }
            }
        }

        if resolved.is_none() {
            for candidate in candidates {
                if let Some(widget) = resolve_icon_widget(
                    &mut self.theme_icon_cache,
                    &self.icon_texture_cache,
                    &candidate,
                    size,
                ) {
                    resolved = Some((candidate, widget));
                    break;
                }
            }
        }

        if let Some((icon_name, widget)) = resolved {
            self.cache_icon(cache_key, Some(icon_name));
            Some(widget)
        } else {
            self.cache_icon(cache_key, None);
            None
        }
    }

    pub(in crate::ui) fn cache_icon(
        &mut self,
        cache_key: IconResolutionKey,
        resolved: Option<String>,
    ) {
        let cached = IconCacheEntry {
            resolved,
            cached_at: Instant::now(),
        };
        // Bound the icon cache to avoid unbounded growth in long-running sessions
        match self.icon_cache.entry(cache_key) {
            std::collections::hash_map::Entry::Occupied(mut entry) => {
                entry.insert(cached);
                return;
            }
            std::collections::hash_map::Entry::Vacant(entry) => {
                let key = entry.key().clone();
                entry.insert(cached);
                self.icon_cache_order.push_back(key);
            }
        }
        let excess_entries = self
            .icon_cache_order
            .len()
            .saturating_sub(ICON_CACHE_MAX_ENTRIES);
        for _ in 0..excess_entries {
            if let Some(evicted) = self.icon_cache_order.pop_front() {
                self.icon_cache.remove(&evicted);
            }
        }
    }

    pub(in crate::ui) fn invalidate_icon_sources(&mut self) {
        // Rebuild both lookup layers so changed desktop entries are resolved again
        self.icon_source_generation = self.icon_source_generation.wrapping_add(1);
        self.desktop_icons.rebuild();
        self.icon_cache.clear();
        self.icon_cache_order.clear();
        self.theme_icon_cache.clear();
        self.icon_texture_cache.borrow_mut().clear();
        self.icon_sources_dirty.set(false);
    }

    pub(in crate::ui) fn refresh_icon_sources_if_needed(&mut self) {
        if self.icon_sources_dirty.replace(false) {
            self.invalidate_icon_sources();
        }
    }
}

fn resolve_icon_widget(
    theme_icon_cache: &mut ThemeIconCache,
    icon_texture_cache: &Rc<std::cell::RefCell<TextureCache>>,
    name: &str,
    size: i32,
) -> Option<gtk::Image> {
    if let Some(file_path) = file_path_from_hint(name) {
        // Decoded file:// paths allow loading icon files with escaped characters
        if file_path.is_file() {
            // Reuse a cached texture when available to avoid repeated decode work
            if let Some(texture) = icon_texture_cache.borrow_mut().get(&file_path, size) {
                let widget = gtk::Image::new();
                widget.set_paintable(Some(&texture));
                set_popup_icon_size(&widget, size);
                return Some(widget);
            }
            return Some(spawn_file_icon(icon_texture_cache, file_path, size));
        }
    }
    // Keep the existing lookup scale so caching does not change rendered icon selection
    let paintable = theme_icon_cache.get_or_resolve(name, size, 1)?;
    let widget = gtk::Image::from_paintable(Some(&paintable));
    set_popup_icon_size(&widget, size);
    Some(widget)
}

fn spawn_file_icon(
    icon_texture_cache: &Rc<std::cell::RefCell<TextureCache>>,
    path: PathBuf,
    size: i32,
) -> gtk::Image {
    let widget = gtk::Image::new();
    set_popup_icon_size(&widget, size);
    let (tx, rx) = async_channel::bounded::<IconDecodeResult>(1);
    let widget_clone = widget.clone();
    let cache = Rc::clone(icon_texture_cache);
    let path_clone = path.clone();
    let target_size = size.max(1);
    // Apply the texture on the main loop to avoid GTK thread violations
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
                    widget_clone.set_paintable(Some(&texture));
                    set_popup_icon_size(&widget_clone, target_size);
                    // Cache only modestly sized textures to limit resident memory
                    if icon.bytes.len() <= ICON_TEXTURE_CACHE_MAX_BYTES {
                        cache.borrow_mut().insert(path_clone, target_size, texture);
                    }
                }
                Err(err) => {
                    debug!(?err, "popup icon decode failed");
                }
            }
        }
    });

    // Decode on a background worker pool to avoid spawning unbounded threads
    IconDecodePool::global().submit(path, target_size, tx);

    widget
}

fn negative_cache_is_fresh(cached_at: Instant, now: Instant) -> bool {
    now.saturating_duration_since(cached_at) < NEGATIVE_ICON_CACHE_TTL
}

#[cfg(test)]
#[path = "tests/state.rs"]
mod tests;

fn set_popup_icon_size(widget: &gtk::Image, size: i32) {
    let size = size.max(1);
    // Enforce a fixed icon footprint so file-backed and themed icons align
    widget.set_pixel_size(size);
    widget.set_size_request(size, size);
}
