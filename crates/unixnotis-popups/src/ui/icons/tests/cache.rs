use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use gtk::gdk;
use gtk::glib;
use gtk::glib::object::Cast;

use super::{IconDecodeJob, IconDecodePool, IconRequestKey, TextureCache};

fn icon_decode_pool(capacity: usize) -> (IconDecodePool, async_channel::Receiver<IconDecodeJob>) {
    let (tx, rx) = async_channel::bounded(capacity);
    let pool = IconDecodePool {
        tx,
        in_flight: Arc::new(Mutex::new(HashMap::new())),
    };
    (pool, rx)
}

fn waiter_count(pool: &IconDecodePool, path: &Path, target_size: i32) -> usize {
    let in_flight = pool
        .in_flight
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    in_flight
        .get(&IconRequestKey::new(path.to_path_buf(), target_size))
        .map_or(0, Vec::len)
}

#[test]
fn icon_decode_deduplicates_in_flight_requests() {
    let (pool, _worker_rx) = icon_decode_pool(4);
    let path = PathBuf::from("icon-test.png");
    let (tx_a, _rx_a) = async_channel::bounded(1);
    let (tx_b, _rx_b) = async_channel::bounded(1);

    pool.submit(path.clone(), 20, tx_a);
    pool.submit(path.clone(), 20, tx_b);

    assert_eq!(pool.tx.len(), 1);
    assert_eq!(waiter_count(&pool, &path, 20), 2);
}

#[test]
fn icon_decode_keeps_different_sizes_separate() {
    let (pool, _worker_rx) = icon_decode_pool(4);
    let path = PathBuf::from("icon-test.png");
    let (tx_a, _rx_a) = async_channel::bounded(1);
    let (tx_b, _rx_b) = async_channel::bounded(1);

    pool.submit(path.clone(), 20, tx_a);
    pool.submit(path.clone(), 32, tx_b);

    assert_eq!(pool.tx.len(), 2);
    assert_eq!(waiter_count(&pool, &path, 20), 1);
    assert_eq!(waiter_count(&pool, &path, 32), 1);
}

#[test]
fn icon_decode_queue_overflow_notifies_waiters() {
    let (pool, _worker_rx) = icon_decode_pool(1);
    let path_a = PathBuf::from("icon-a.png");
    let path_b = PathBuf::from("icon-b.png");
    let (tx_a, _rx_a) = async_channel::bounded(1);
    let (tx_b, rx_b) = async_channel::bounded(1);

    pool.submit(path_a, 20, tx_a);
    pool.submit(path_b.clone(), 20, tx_b);

    let result = rx_b.recv_blocking().expect("reply expected");
    assert!(result.is_err());
    assert_eq!(waiter_count(&pool, &path_b, 20), 0);
}

#[gtk::test]
fn texture_cache_keeps_sizes_separate() {
    let mut cache = TextureCache::new(4);
    let path = PathBuf::from("icon-test.png");
    let bytes = glib::Bytes::from_owned(vec![255; 4]);
    let small = gdk::MemoryTexture::new(1, 1, gdk::MemoryFormat::R8g8b8a8, &bytes, 4)
        .upcast::<gdk::Texture>();
    let large = gdk::MemoryTexture::new(1, 1, gdk::MemoryFormat::R8g8b8a8, &bytes, 4)
        .upcast::<gdk::Texture>();

    cache.insert(path.clone(), 20, small);
    cache.insert(path.clone(), 32, large);

    assert!(cache.get(&path, 20).is_some());
    assert!(cache.get(&path, 32).is_some());
}
