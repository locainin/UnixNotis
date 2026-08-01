//! Cache and key management for notification icons
//!
//! Encapsulates cache storage and keying logic used by the icon resolver

use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::rc::Rc;

use gtk::gdk::{Paintable, Texture};
use gtk::prelude::*;
use gtk::IconPaintable;
use unixnotis_core::NotificationImage;

const DEFAULT_MAX_CACHE_BYTES: usize = 64 * 1024 * 1024;

// Thread-local storage replaces glib qdata to avoid unsafe pointer casts
// The map key is the raw GObject pointer; entries persist for the widget lifetime
// and stale entries are harmless because the bound IconKey values are small
thread_local! {
    static IMAGE_KEYS: RefCell<HashMap<*const (), IconKey>> = RefCell::new(HashMap::new());
}

fn glib_ptr<T: gtk::glib::prelude::ObjectType>(obj: &T) -> *const () {
    // Extract the raw GObject pointer for use as a thread-local HashMap key
    // This replaces glib qdata with safe Rust storage while preserving identity
    obj.as_ptr() as *const ()
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub(super) enum IconKey {
    ImageData {
        hash: [u8; 32],
        len: usize,
        width: i32,
        height: i32,
        size: i32,
        scale: i32,
    },
    Path {
        path: PathBuf,
        size: i32,
        scale: i32,
    },
    Name {
        name: String,
        size: i32,
        scale: i32,
    },
}

impl IconKey {
    pub(super) const fn size_and_scale(&self) -> (i32, i32) {
        match self {
            Self::ImageData { size, scale, .. }
            | Self::Path { size, scale, .. }
            | Self::Name { size, scale, .. } => (*size, *scale),
        }
    }
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
    // Empty path means “no icon path provided”; treat as absent rather than creating a useless cache key
    if path.as_os_str().is_empty() {
        return None;
    }

    // PathBuf preserves every Unix filename byte so distinct files cannot share a cache entry
    Some(IconKey::Path {
        path: path.to_path_buf(),
        size,  // Target icon size in logical pixels (used to avoid cross-size cache collisions).
        scale, // Output scale factor (used to avoid mixing 1x/2x assets in the same entry).
    })
}

pub(super) fn icon_key_for_name(name: &str, size: i32, scale: i32) -> Option<IconKey> {
    // Empty icon name means “no themed icon requested”; treat as absent
    if name.is_empty() {
        return None;
    }

    // Store an owned copy of the name so the key outlives the caller's &str
    // Size/scale are included so the same themed icon can be cached distinctly per requested resolution
    Some(IconKey::Name {
        name: name.to_string(),
        size,
        scale,
    })
}

fn hash_image_data(data: &[u8]) -> [u8; 32] {
    // Notification image payloads are already bounded, so hashing every byte keeps cache identity exact
    *blake3::hash(data).as_bytes()
}

pub(super) fn set_image_key(image: &gtk::Image, key: IconKey) {
    // Store the icon key in thread-local storage keyed by the GObject pointer
    // This replaces glib::ObjectExt::set_qdata to keep the codebase free of unsafe blocks
    IMAGE_KEYS.with(|map| {
        map.borrow_mut().insert(glib_ptr(image), key);
    });
}

pub(super) fn image_key_matches(image: &gtk::Image, key: &IconKey) -> bool {
    // Retrieve the stored icon key from thread-local storage by GObject pointer
    // Returns false when no key was stored or the stored key differs from the request
    IMAGE_KEYS.with(|map| {
        let map = map.borrow();
        map.get(&glib_ptr(image)) == Some(key)
    })
}

#[cfg(test)]
#[path = "tests/cache.rs"]
mod tests;

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

    pub(super) const fn from_texture(texture: Texture) -> Self {
        Self {
            inner: CachedPaintableInner::Texture(texture),
        }
    }

    pub(super) const fn from_icon(icon: IconPaintable) -> Self {
        Self {
            inner: CachedPaintableInner::Icon(icon),
        }
    }
}

pub(super) struct IconCache {
    entries: HashMap<IconKey, CacheEntry>,
    order: VecDeque<IconKey>,
    max_entries: usize,
    max_bytes: usize,
    total_bytes: usize,
}

impl IconCache {
    pub(super) fn new(max_entries: usize) -> Self {
        // Create an empty bounded cache. The cache is keyed by IconKey and stores Rc<CachedPaintable>
        // so callers can cheaply clone references without copying the underlying paintable
        //
        // order is a simple LRU-like list (oldest at front, newest at back)
        Self {
            entries: HashMap::new(),            // Key -> cached paintable (shared via Rc)
            order: VecDeque::new(),             // Recency order for eviction / promotion
            max_entries,                        // Maximum number of entries we keep before evicting
            max_bytes: DEFAULT_MAX_CACHE_BYTES, // Approximate memory budget for cached textures.
            total_bytes: 0,
        }
    }

    pub(super) fn get(&mut self, key: &IconKey) -> Option<Rc<CachedPaintable>> {
        // Fast path: look up by key. If present, clone the Rc (cheap) and promote in LRU order
        // We take &mut self because promotion mutates the recency list
        let paintable = self.entries.get(key)?.paintable.clone();

        // Mark this key as most-recently used so it is less likely to be evicted
        self.promote(key);

        Some(paintable)
    }

    pub(super) fn insert(
        &mut self,
        key: IconKey,
        paintable: CachedPaintable,
    ) -> Rc<CachedPaintable> {
        // Wrap the paintable in Rc so it can be shared by multiple widgets without copying
        let paintable = Rc::new(paintable);
        let estimated_bytes = estimate_cache_bytes(&paintable, &key);

        // Insert/replace in the map. If this key already existed, this overwrites the value
        // Ensure order stays bounded by removing any existing entry before re-adding
        if let Some(entry) = self.entries.insert(
            key.clone(),
            CacheEntry {
                paintable: paintable.clone(),
                bytes: estimated_bytes,
            },
        ) {
            self.total_bytes = self.total_bytes.saturating_sub(entry.bytes);
        }
        self.total_bytes = self.total_bytes.saturating_add(estimated_bytes);

        // Record as most-recently used
        self.order.retain(|item| item != &key);
        self.order.push_back(key);

        // Enforce size bound (evicts least-recently used items)
        self.evict();

        paintable
    }

    fn promote(&mut self, key: &IconKey) {
        // Promote the key in the recency deque:
        // - find its current position
        // - remove it from that spot
        // - push it to the back (most-recently used)
        //
        // This is O(n) due to position search; for small max_entries this is fine
        // If max_entries grows large, consider an LRU structure with a linked map
        if let Some(position) = self.order.iter().position(|item| item == key) {
            if let Some(item) = self.order.remove(position) {
                self.order.push_back(item);
            }
        }
    }

    fn evict(&mut self) {
        // Trim the oldest entries to keep cache memory bounded
        // Oldest == front of the deque. Newest == back of the deque
        while self.entries.len() > self.max_entries || self.total_bytes > self.max_bytes {
            if let Some(key) = self.order.pop_front() {
                // Remove the entry from the map as well. If order contains duplicates (possible when
                // inserting the same key multiple times), removing here might no-op if it was already
                // removed earlier; that's safe, and the loop will continue trimming until bounded
                if let Some(entry) = self.entries.remove(&key) {
                    self.total_bytes = self.total_bytes.saturating_sub(entry.bytes);
                }
            } else {
                // order should normally track entries, but if it gets out of sync,
                // break to avoid an infinite loop
                break;
            }
        }
    }
}

#[derive(Clone)]
struct CacheEntry {
    paintable: Rc<CachedPaintable>,
    bytes: usize,
}

fn estimate_cache_bytes(paintable: &CachedPaintable, key: &IconKey) -> usize {
    match &paintable.inner {
        CachedPaintableInner::Texture(texture) => {
            let width = texture.width().max(1) as usize;
            let height = texture.height().max(1) as usize;
            width.saturating_mul(height).saturating_mul(4)
        }
        CachedPaintableInner::Icon(_) => {
            let (size, scale) = key.size_and_scale();
            let scale = scale.max(1);
            let pixels = (size.max(1) * scale) as usize;
            pixels.saturating_mul(pixels).saturating_mul(4)
        }
    }
}
