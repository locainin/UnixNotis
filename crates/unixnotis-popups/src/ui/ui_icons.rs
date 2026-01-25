//! Icon decode, cache, and widget construction for popups.
//!
//! Keeps icon decoding, caching, and texture reuse isolated from UI state handling.

use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;

use gtk::glib::object::Cast;
use gtk::{gdk, glib};
use tracing::{debug, warn};
use unixnotis_core::NotificationView;

use super::icons::{
    collect_icon_candidates, decode_icon_file, file_path_from_hint, image_data_texture,
    resolve_icon_image, RasterIcon,
};
use super::UiState;

const ICON_CACHE_MAX_ENTRIES: usize = 256;
// Limit cached textures to keep memory use predictable on long-running sessions.
const ICON_TEXTURE_CACHE_MAX_ENTRIES: usize = 64;
// Skip caching decoded textures above this size to avoid holding large buffers.
const ICON_TEXTURE_CACHE_MAX_BYTES: usize = 1024 * 1024;
const ICON_DECODE_WORKERS: usize = 2;
// Limit queued decode jobs to keep bursts from accumulating unbounded memory.
const ICON_DECODE_QUEUE_CAPACITY: usize = 64;

enum IconDecodeDropPolicy {
    DropNewest,
}

// Bounded queues rely on an explicit drop policy for overload behavior.
const ICON_DECODE_DROP_POLICY: IconDecodeDropPolicy = IconDecodeDropPolicy::DropNewest;

struct IconDecodeJob {
    path: PathBuf,
}

// Arc shares decoded bytes across waiters without cloning large buffers.
type IconDecodeResult = Result<Arc<RasterIcon>, String>;
type IconReply = async_channel::Sender<IconDecodeResult>;
type IconWaiters = Arc<Mutex<HashMap<PathBuf, Vec<IconReply>>>>;

struct IconDecodePool {
    tx: async_channel::Sender<IconDecodeJob>,
    in_flight: IconWaiters,
    // Test-only receiver guard keeps the channel open when no workers are spawned.
    #[cfg(test)]
    #[allow(dead_code)]
    rx_guard: async_channel::Receiver<IconDecodeJob>,
}

impl IconDecodePool {
    fn global() -> &'static IconDecodePool {
        static POOL: OnceLock<IconDecodePool> = OnceLock::new();
        POOL.get_or_init(|| IconDecodePool::new(ICON_DECODE_WORKERS))
    }

    fn new(worker_count: usize) -> Self {
        Self::new_with_capacity(worker_count, ICON_DECODE_QUEUE_CAPACITY, true)
    }

    fn new_with_capacity(worker_count: usize, capacity: usize, spawn_workers: bool) -> Self {
        // Keep the decode queue bounded to prevent unbounded memory growth on bursts.
        let (tx, rx) = async_channel::bounded::<IconDecodeJob>(capacity);
        let in_flight: IconWaiters = Arc::new(Mutex::new(HashMap::new()));
        #[cfg(test)]
        let rx_guard = rx.clone();
        if spawn_workers {
            // Limit decode concurrency to keep bursty icon loads from spawning unbounded threads.
            for idx in 0..worker_count.max(1) {
                let rx = rx.clone();
                let in_flight = Arc::clone(&in_flight);
                let name = format!("unixnotis-icon-decode-{idx}");
                if thread::Builder::new()
                    .name(name)
                    .spawn(move || worker_loop(rx, in_flight))
                    .is_err()
                {
                    warn!("failed to spawn icon decode worker");
                }
            }
        }
        Self {
            tx,
            in_flight,
            #[cfg(test)]
            rx_guard,
        }
    }

    fn submit(&self, path: PathBuf, reply: IconReply) {
        // Deduplicate in-flight requests so repeated icon paths share a single decode.
        {
            let mut in_flight = match self.in_flight.lock() {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };
            if let Some(waiters) = in_flight.get_mut(&path) {
                waiters.push(reply);
                return;
            }
            in_flight.insert(path.clone(), vec![reply]);
        }

        // Avoid blocking the GTK thread; drop on overflow and signal failure to the caller.
        match self.tx.try_send(IconDecodeJob { path: path.clone() }) {
            Ok(()) => {}
            Err(async_channel::TrySendError::Full(job)) => {
                let reason = match ICON_DECODE_DROP_POLICY {
                    IconDecodeDropPolicy::DropNewest => "icon decode queue full (drop-newest)",
                };
                self.drop_in_flight(&job.path, reason);
            }
            Err(async_channel::TrySendError::Closed(job)) => {
                self.drop_in_flight(&job.path, "icon decode queue closed");
            }
        }
    }

    fn drop_in_flight(&self, path: &PathBuf, reason: &str) {
        let waiters = {
            let mut in_flight = match self.in_flight.lock() {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };
            in_flight.remove(path)
        };
        let Some(waiters) = waiters else {
            return;
        };
        for waiter in waiters {
            let _ = waiter.try_send(Err(reason.to_string()));
        }
        if matches!(ICON_DECODE_DROP_POLICY, IconDecodeDropPolicy::DropNewest) {
            debug!(path = ?path, "dropped newest icon decode request");
        }
    }
}

#[cfg(test)]
impl IconDecodePool {
    fn new_for_tests(worker_count: usize, capacity: usize) -> Self {
        Self::new_with_capacity(worker_count, capacity, false)
    }

    fn queue_len(&self) -> usize {
        self.tx.len()
    }

    fn waiter_count(&self, path: &PathBuf) -> usize {
        let in_flight = match self.in_flight.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        in_flight
            .get(path)
            .map(|waiters| waiters.len())
            .unwrap_or(0)
    }
}

// Small LRU cache for decoded file textures to avoid repeated decoding.
pub(super) struct TextureCache {
    entries: HashMap<PathBuf, gdk::Texture>,
    order: VecDeque<PathBuf>,
    max_entries: usize,
}

impl TextureCache {
    fn new(max_entries: usize) -> Self {
        Self {
            entries: HashMap::new(),
            order: VecDeque::new(),
            max_entries,
        }
    }

    pub(super) fn new_for_popups() -> Self {
        // Use a small cache to keep common icons hot without unbounded memory use.
        Self::new(ICON_TEXTURE_CACHE_MAX_ENTRIES)
    }

    fn get(&mut self, path: &Path) -> Option<gdk::Texture> {
        let texture = self.entries.get(path).cloned();
        if texture.is_some() {
            self.bump(path);
        }
        texture
    }

    fn insert(&mut self, path: PathBuf, texture: gdk::Texture) {
        if self.entries.contains_key(&path) {
            self.entries.insert(path.clone(), texture);
            self.bump(&path);
            return;
        }

        self.entries.insert(path.clone(), texture);
        self.order.push_back(path.clone());
        self.enforce_limit();
    }

    fn bump(&mut self, path: &Path) {
        // Move the key to the back to reflect recent use.
        if let Some(pos) = self.order.iter().position(|entry| entry == path) {
            let key = self.order.remove(pos).expect("position checked");
            self.order.push_back(key);
        }
    }

    fn enforce_limit(&mut self) {
        while self.order.len() > self.max_entries {
            if let Some(evicted) = self.order.pop_front() {
                self.entries.remove(&evicted);
            }
        }
    }
}

fn worker_loop(rx: async_channel::Receiver<IconDecodeJob>, in_flight: IconWaiters) {
    while let Ok(job) = rx.recv_blocking() {
        // Decode file-backed icons off the GTK thread to keep animations smooth.
        let result = decode_icon_file(&job.path).map(Arc::new);
        let waiters = {
            let mut in_flight = match in_flight.lock() {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };
            in_flight.remove(&job.path)
        };
        let Some(waiters) = waiters else {
            continue;
        };
        for waiter in waiters {
            let _ = waiter.send_blocking(result.clone());
        }
    }
}

impl UiState {
    pub(super) fn build_image_widget(
        &mut self,
        notification: &NotificationView,
    ) -> Option<gtk::Image> {
        let image = &notification.image;
        if let Some(texture) = image_data_texture(image) {
            let widget = gtk::Image::from_paintable(Some(&texture));
            widget.set_pixel_size(20);
            return Some(widget);
        }

        if !image.image_path.is_empty() {
            let path = image.image_path.as_str();
            if let Some(file_path) = file_path_from_hint(path) {
                // Decoded file:// paths allow loading icon files with escaped characters.
                if file_path.is_file() {
                    // Reuse a cached texture when available to avoid repeated decode work.
                    if let Some(texture) = self.icon_texture_cache.borrow_mut().get(&file_path) {
                        let widget = gtk::Image::new();
                        widget.set_paintable(Some(&texture));
                        return Some(widget);
                    }
                    return Some(self.spawn_file_icon(file_path));
                }
            }
            return resolve_icon_image(path, 20);
        }

        let cache_key = format!("{}|{}", notification.app_name, notification.image.icon_name);
        if let Some(cached) = self.icon_cache.get(&cache_key) {
            return cached
                .as_ref()
                .and_then(|icon_name| resolve_icon_image(icon_name, 20));
        }

        let candidates = collect_icon_candidates(notification);
        // Keep the first successful resolve to avoid duplicate theme lookups and widget creation.
        let mut resolved: Option<(String, gtk::Image)> = None;

        for candidate in &candidates {
            if let Some(icon_names) = self.desktop_icons.icons_for(candidate) {
                for icon_name in icon_names {
                    if let Some(widget) = resolve_icon_image(icon_name.as_str(), 20) {
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
                if let Some(widget) = resolve_icon_image(candidate, 20) {
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

    fn spawn_file_icon(&self, path: PathBuf) -> gtk::Image {
        let widget = gtk::Image::new();
        let (tx, rx) = async_channel::bounded::<IconDecodeResult>(1);
        let widget_clone = widget.clone();
        let cache = self.icon_texture_cache.clone();
        let path_clone = path.clone();
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
                    }
                    Err(err) => {
                        debug!(?err, "popup icon decode failed");
                    }
                }
            }
        });

        // Decode on a background worker pool to avoid spawning unbounded threads.
        IconDecodePool::global().submit(path, tx);

        widget
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn icon_decode_deduplicates_in_flight_requests() {
        let pool = IconDecodePool::new_for_tests(0, 4);
        let path = PathBuf::from("icon-test.png");
        let (tx_a, _rx_a) = async_channel::bounded(1);
        let (tx_b, _rx_b) = async_channel::bounded(1);

        pool.submit(path.clone(), tx_a);
        pool.submit(path.clone(), tx_b);

        assert_eq!(pool.queue_len(), 1);
        assert_eq!(pool.waiter_count(&path), 2);
    }

    #[test]
    fn icon_decode_queue_overflow_notifies_waiters() {
        let pool = IconDecodePool::new_for_tests(0, 1);
        let path_a = PathBuf::from("icon-a.png");
        let path_b = PathBuf::from("icon-b.png");
        let (tx_a, _rx_a) = async_channel::bounded(1);
        let (tx_b, rx_b) = async_channel::bounded(1);

        pool.submit(path_a, tx_a);
        pool.submit(path_b.clone(), tx_b);

        let result = rx_b.recv_blocking().expect("reply expected");
        assert!(result.is_err());
        assert_eq!(pool.waiter_count(&path_b), 0);
    }
}
