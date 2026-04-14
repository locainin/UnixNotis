//! Popup icon decode queue and texture cache
//!
//! Keeps background decode state away from the popup UI module

use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;

use gtk::gdk;
use tracing::{debug, warn};

use super::{decode_icon_file, RasterIcon};

const ICON_DECODE_WORKERS: usize = 2;
// Limit queued decode jobs to keep bursts from accumulating unbounded memory
const ICON_DECODE_QUEUE_CAPACITY: usize = 64;
// Limit cached textures to keep memory use predictable on long-running sessions
const ICON_TEXTURE_CACHE_MAX_ENTRIES: usize = 64;

enum IconDecodeDropPolicy {
    DropNewest,
}

// Bounded queues rely on an explicit drop policy for overload behavior
const ICON_DECODE_DROP_POLICY: IconDecodeDropPolicy = IconDecodeDropPolicy::DropNewest;

struct IconDecodeJob {
    path: PathBuf,
    target_size: i32,
}

// Arc shares decoded bytes across waiters without cloning large buffers
pub(crate) type IconDecodeResult = Result<Arc<RasterIcon>, String>;
type IconReply = async_channel::Sender<IconDecodeResult>;
type IconWaiters = Arc<Mutex<HashMap<PathBuf, Vec<IconReply>>>>;

pub(crate) struct IconDecodePool {
    tx: async_channel::Sender<IconDecodeJob>,
    in_flight: IconWaiters,
    // Test-only receiver guard keeps the channel open when no workers are spawned
    #[cfg(test)]
    #[allow(dead_code)]
    rx_guard: async_channel::Receiver<IconDecodeJob>,
}

impl IconDecodePool {
    pub(crate) fn global() -> &'static IconDecodePool {
        // One shared pool is enough for the popup process
        static POOL: OnceLock<IconDecodePool> = OnceLock::new();
        POOL.get_or_init(|| IconDecodePool::new(ICON_DECODE_WORKERS))
    }

    fn new(worker_count: usize) -> Self {
        Self::new_with_capacity(worker_count, ICON_DECODE_QUEUE_CAPACITY, true)
    }

    fn new_with_capacity(worker_count: usize, capacity: usize, spawn_workers: bool) -> Self {
        // Keep the decode queue bounded to prevent unbounded memory growth on bursts
        let (tx, rx) = async_channel::bounded::<IconDecodeJob>(capacity);
        let in_flight: IconWaiters = Arc::new(Mutex::new(HashMap::new()));
        #[cfg(test)]
        let rx_guard = rx.clone();
        if spawn_workers {
            // Limit decode concurrency to keep bursty icon loads from spawning unbounded threads
            for idx in 0..worker_count.max(1) {
                let rx = rx.clone();
                let in_flight = Arc::clone(&in_flight);
                let name = format!("unixnotis-icon-decode-{idx}");
                if thread::Builder::new()
                    .name(name)
                    .spawn(move || worker_loop(rx, in_flight))
                    .is_err()
                {
                    // Failed workers are logged and the queue still stays bounded
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

    pub(crate) fn submit(&self, path: PathBuf, target_size: i32, reply: IconReply) {
        // Deduplicate in-flight requests so repeated icon paths share a single decode
        {
            let mut in_flight = match self.in_flight.lock() {
                Ok(guard) => guard,
                // Poisoned mutexes still give back the stored waiters
                Err(poisoned) => poisoned.into_inner(),
            };
            if let Some(waiters) = in_flight.get_mut(&path) {
                // Extra callers wait on the first decode instead of queuing another job
                waiters.push(reply);
                return;
            }
            // First caller owns the actual worker submission
            in_flight.insert(path.clone(), vec![reply]);
        }

        // Avoid blocking the GTK thread; drop on overflow and signal failure to the caller
        match self.tx.try_send(IconDecodeJob {
            path: path.clone(),
            target_size,
        }) {
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
        // Pull the waiter list out first so sends happen without holding the mutex
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
            // Overflow and shutdown paths report the same error back to all waiters
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

// Small LRU cache for decoded file textures to avoid repeated decoding
pub(crate) struct TextureCache {
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

    pub(crate) fn new_for_popups() -> Self {
        // Use a small cache to keep common icons hot without unbounded memory use
        Self::new(ICON_TEXTURE_CACHE_MAX_ENTRIES)
    }

    pub(crate) fn get(&mut self, path: &Path) -> Option<gdk::Texture> {
        let texture = self.entries.get(path).cloned();
        if texture.is_some() {
            // Hits move to the back so hot icons stay cached
            self.bump(path);
        }
        texture
    }

    pub(crate) fn insert(&mut self, path: PathBuf, texture: gdk::Texture) {
        if self.entries.contains_key(&path) {
            // Replacing the texture also refreshes the recency position
            self.entries.insert(path.clone(), texture);
            self.bump(&path);
            return;
        }

        // First insert keeps the same key in the map and the LRU queue
        self.entries.insert(path.clone(), texture);
        self.order.push_back(path.clone());
        self.enforce_limit();
    }

    fn bump(&mut self, path: &Path) {
        // Move the key to the back to reflect recent use
        if let Some(pos) = self.order.iter().position(|entry| entry == path) {
            let key = self.order.remove(pos).expect("position checked");
            self.order.push_back(key);
        }
    }

    fn enforce_limit(&mut self) {
        while self.order.len() > self.max_entries {
            if let Some(evicted) = self.order.pop_front() {
                // Evicted keys are removed from the texture map at the same time
                self.entries.remove(&evicted);
            }
        }
    }
}

fn worker_loop(rx: async_channel::Receiver<IconDecodeJob>, in_flight: IconWaiters) {
    while let Ok(job) = rx.recv_blocking() {
        // Decode file-backed icons off the GTK thread to keep animations smooth
        let result = decode_icon_file(&job.path, job.target_size).map(Arc::new);
        let waiters = {
            let mut in_flight = match in_flight.lock() {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };
            // Remove the path before waking waiters so later requests can queue again
            in_flight.remove(&job.path)
        };
        let Some(waiters) = waiters else {
            continue;
        };
        for waiter in waiters {
            // Every waiter gets the same decoded result or error
            let _ = waiter.send_blocking(result.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::IconDecodePool;

    #[test]
    fn icon_decode_deduplicates_in_flight_requests() {
        let pool = IconDecodePool::new_for_tests(0, 4);
        let path = PathBuf::from("icon-test.png");
        let (tx_a, _rx_a) = async_channel::bounded(1);
        let (tx_b, _rx_b) = async_channel::bounded(1);

        pool.submit(path.clone(), 20, tx_a);
        pool.submit(path.clone(), 20, tx_b);

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

        pool.submit(path_a, 20, tx_a);
        pool.submit(path_b.clone(), 20, tx_b);

        let result = rx_b.recv_blocking().expect("reply expected");
        assert!(result.is_err());
        assert_eq!(pool.waiter_count(&path_b), 0);
    }
}
