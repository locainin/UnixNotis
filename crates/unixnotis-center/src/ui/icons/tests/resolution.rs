use std::cell::RefCell;
use std::collections::HashMap;
use std::fs;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use gtk::prelude::*;
use image::codecs::png::PngEncoder;
use image::{ExtendedColorType, ImageEncoder};
use unixnotis_core::{NotificationImage, NotificationView};
use unixnotis_ui::icons::DesktopIconIndex;

use super::{icon_name_is_usable, IconResolverInner};
use crate::ui::icons::cache::{set_image_key, IconCache};
use crate::ui::icons::decode::{IconUpdate, IconWorker};
use crate::ui::icons::missing::MissingIconCache;
use crate::ui::icons::types::IconResolution;

#[test]
fn empty_icon_name_is_not_resolved() {
    assert!(!icon_name_is_usable(""));
}

#[test]
fn nonempty_icon_name_is_resolved_without_rewriting() {
    assert!(icon_name_is_usable("application-x-executable"));
}

fn resolver_inner(update_tx: async_channel::Sender<IconUpdate>) -> IconResolverInner {
    IconResolverInner {
        desktop_index: DesktopIconIndex::new(),
        cache: RefCell::new(IconCache::new(16)),
        inflight: RefCell::new(HashMap::new()),
        missing_names: RefCell::new(MissingIconCache::new(16)),
        worker: IconWorker::new(update_tx),
    }
}

fn test_png() -> Vec<u8> {
    let mut bytes = Vec::new();
    PngEncoder::new(&mut bytes)
        .write_image(&[1, 2, 3, 255], 1, 1, ExtendedColorType::Rgba8)
        .expect("encode icon PNG");
    bytes
}

fn wait_for_update(receiver: &async_channel::Receiver<IconUpdate>) -> IconUpdate {
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        match receiver.try_recv() {
            Ok(update) => return update,
            Err(async_channel::TryRecvError::Closed) => panic!("icon update channel closed"),
            Err(async_channel::TryRecvError::Empty) if Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(5));
            }
            Err(async_channel::TryRecvError::Empty) => panic!("icon worker did not respond"),
        }
    }
}

#[gtk::test]
fn file_icon_resolution_enqueues_decodes_and_applies_the_worker_result() {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "unixnotis-center-resolution-{}-{stamp}.png",
        std::process::id()
    ));
    fs::write(&path, test_png()).expect("write icon fixture");
    let (update_tx, update_rx) = async_channel::bounded(4);
    let resolver = resolver_inner(update_tx);
    let notification = NotificationView {
        id: 1,
        app_name: "Icon test".to_string(),
        summary: String::new(),
        body: String::new(),
        actions: Vec::new(),
        urgency: 1,
        is_transient: false,
        image: NotificationImage {
            image_path: path.to_string_lossy().into_owned(),
            ..NotificationImage::default()
        },
    };

    let resolution = resolver
        .resolve_icon(&notification, 16, 1)
        .expect("file icon should resolve");
    let IconResolution::Async { request } = resolution else {
        panic!("file icon should use the worker");
    };
    let image = gtk::Image::new();
    image.set_visible(false);
    set_image_key(&image, request.key.clone());
    resolver.enqueue(request, &image);

    resolver.handle_update(wait_for_update(&update_rx));

    assert!(image.get_visible());
    assert!(image.paintable().is_some());
    assert!(resolver.inflight.borrow().is_empty());
    fs::remove_file(path).expect("remove icon fixture");
}

#[gtk::test]
fn standard_theme_icon_name_resolves_through_the_resolver() {
    let (update_tx, _update_rx) = async_channel::bounded(1);
    let resolver = resolver_inner(update_tx);

    let resolution = resolver
        .resolve_icon_name("folder", 24, 1)
        .or_else(|| resolver.resolve_icon_name("folder-symbolic", 24, 1));

    assert!(resolution.is_some());
}
