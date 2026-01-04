//! Cache and key management for notification icons.
//!
//! Encapsulates cache storage and keying logic used by the icon resolver.

use std::collections::{HashMap, VecDeque};
use std::hash::{Hash, Hasher};
use std::path::Path;
use std::rc::Rc;

use gtk::gdk::{Paintable, Texture};
use gtk::IconPaintable;
use gtk::prelude::*;
use unixnotis_core::NotificationImage;

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub(super) enum IconKey {
    ImageData {
        hash: u64,
        len: usize,
        width: i32,
        height: i32,
        size: i32,
        scale: i32,
    },
    Path {
        path: String,
        size: i32,
        scale: i32,
    },
    Name {
        name: String,
        size: i32,
        scale: i32,
    },
}

pub(super) fn icon_key_for_image(
    image: &NotificationImage,
    size: i32,
    scale: i32,
) -> Option<IconKey> {
    if !image.has_image_data {
        return None;
    }
    let data = &image.image_data;
    if data.data.is_empty() {
        return None;
    }
    let hash = hash_image_data(&data.data);
    Some(IconKey::ImageData {
        hash,
        len: data.data.len(),
        width: data.width,
        height: data.height,
        size,
        scale,
    })
}

pub(super) fn icon_key_for_path(path: &Path, size: i32, scale: i32) -> Option<IconKey> {
    if path.as_os_str().is_empty() {
        return None;
    }
    Some(IconKey::Path {
        path: path.to_string_lossy().to_string(),
        size,
        scale,
    })
}

pub(super) fn icon_key_for_name(name: &str, size: i32, scale: i32) -> Option<IconKey> {
    if name.is_empty() {
        return None;
    }
    Some(IconKey::Name {
        name: name.to_string(),
        size,
        scale,
    })
}

fn hash_image_data(data: &[u8]) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    data.len().hash(&mut hasher);
    let sample = 64.min(data.len());
    data[..sample].hash(&mut hasher);
    if data.len() > sample {
        data[data.len() - sample..].hash(&mut hasher);
    }
    hasher.finish()
}

pub(super) fn set_image_key(image: &gtk::Image, key: IconKey) {
    unsafe {
        image.set_data("unixnotis-icon-key", key);
    }
}

pub(super) fn image_key_matches(image: &gtk::Image, key: &IconKey) -> bool {
    unsafe {
        image
            .data::<IconKey>("unixnotis-icon-key")
            .map(|ptr| ptr.as_ref() == key)
            .unwrap_or(false)
    }
}

#[derive(Clone)]
pub(super) struct CachedPaintable {
    inner: CachedPaintableInner,
}

#[derive(Clone)]
enum CachedPaintableInner {
    Texture(Texture),
    Icon(IconPaintable),
}

impl CachedPaintable {
    pub(super) fn paintable(&self) -> &Paintable {
        match &self.inner {
            CachedPaintableInner::Texture(texture) => texture.upcast_ref(),
            CachedPaintableInner::Icon(icon) => icon.upcast_ref(),
        }
    }

    pub(super) fn from_texture(texture: Texture) -> Self {
        Self {
            inner: CachedPaintableInner::Texture(texture),
        }
    }

    pub(super) fn from_icon(icon: IconPaintable) -> Self {
        Self {
            inner: CachedPaintableInner::Icon(icon),
        }
    }
}

pub(super) struct IconCache {
    entries: HashMap<IconKey, Rc<CachedPaintable>>,
    order: VecDeque<IconKey>,
    max_entries: usize,
}

impl IconCache {
    pub(super) fn new(max_entries: usize) -> Self {
        Self {
            entries: HashMap::new(),
            order: VecDeque::new(),
            max_entries,
        }
    }

    pub(super) fn get(&mut self, key: &IconKey) -> Option<Rc<CachedPaintable>> {
        let paintable = self.entries.get(key)?.clone();
        self.promote(key);
        Some(paintable)
    }

    pub(super) fn insert(&mut self, key: IconKey, paintable: CachedPaintable) -> Rc<CachedPaintable> {
        let paintable = Rc::new(paintable);
        self.entries.insert(key.clone(), paintable.clone());
        self.order.push_back(key);
        self.evict();
        paintable
    }

    fn promote(&mut self, key: &IconKey) {
        if let Some(position) = self.order.iter().position(|item| item == key) {
            if let Some(item) = self.order.remove(position) {
                self.order.push_back(item);
            }
        }
    }

    fn evict(&mut self) {
        // Trim the oldest entries to keep cache memory bounded.
        while self.entries.len() > self.max_entries {
            if let Some(key) = self.order.pop_front() {
                self.entries.remove(&key);
            } else {
                break;
            }
        }
    }
}
