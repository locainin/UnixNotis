use std::path::PathBuf;

use gtk::gdk;
use gtk::glib;
use gtk::glib::object::Cast;

use super::{IconDecodePool, TextureCache};

#[test]
fn icon_decode_deduplicates_in_flight_requests() {
    let pool = IconDecodePool::new_for_tests(0, 4);
    let path = PathBuf::from("icon-test.png");
    let (tx_a, _rx_a) = async_channel::bounded(1);
    let (tx_b, _rx_b) = async_channel::bounded(1);

    pool.submit(path.clone(), 20, tx_a);
    pool.submit(path.clone(), 20, tx_b);

    assert_eq!(pool.queue_len(), 1);
    assert_eq!(pool.waiter_count(&path, 20), 2);
}

#[test]
fn icon_decode_keeps_different_sizes_separate() {
    let pool = IconDecodePool::new_for_tests(0, 4);
    let path = PathBuf::from("icon-test.png");
    let (tx_a, _rx_a) = async_channel::bounded(1);
    let (tx_b, _rx_b) = async_channel::bounded(1);

    pool.submit(path.clone(), 20, tx_a);
    pool.submit(path.clone(), 32, tx_b);

    assert_eq!(pool.queue_len(), 2);
    assert_eq!(pool.waiter_count(&path, 20), 1);
    assert_eq!(pool.waiter_count(&path, 32), 1);
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
    assert_eq!(pool.waiter_count(&path_b, 20), 0);
}

#[test]
fn texture_cache_keeps_sizes_separate() {
    let _guard = crate::test_support::gtk_test_lock();
    let mut cache = TextureCache::new(4);
    let path = PathBuf::from("icon-test.png");
    let bytes = glib::Bytes::from_owned(vec![255; 4]);
    let small = gdk::MemoryTexture::new(1, 1, gdk::MemoryFormat::R8g8b8a8, &bytes, 4)
        .upcast::<gdk::Texture>();
    let large = gdk::MemoryTexture::new(1, 1, gdk::MemoryFormat::R8g8b8a8, &bytes, 4)
        .upcast::<gdk::Texture>();

    cache.insert(path.clone(), 20, small.clone());
    cache.insert(path.clone(), 32, large.clone());

    assert!(cache.get(&path, 20).is_some());
    assert!(cache.get(&path, 32).is_some());
}
