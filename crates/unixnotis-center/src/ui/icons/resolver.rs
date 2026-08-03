//! Public icon resolver and shared resolver state

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use gtk::glib;
use unixnotis_core::NotificationView;
use unixnotis_ui::icons::DesktopIconIndex;

use super::cache::{IconCache, IconKey};
use super::decode::{IconUpdate, IconWorker};
use super::missing::MissingIconCache;

// Update buffering remains bounded when GTK is busy
const ICON_UPDATE_QUEUE_CAPACITY: usize = 256;

/// Resolves notification icons using image hints, themed icons, and desktop metadata
pub struct IconResolver {
    inner: Rc<IconResolverInner>,
}

impl IconResolver {
    pub fn new() -> Self {
        // Worker results return through one bounded main-loop channel
        let (update_tx, update_rx) =
            async_channel::bounded::<IconUpdate>(ICON_UPDATE_QUEUE_CAPACITY);
        let worker = IconWorker::new(update_tx);
        let inner = Rc::new(IconResolverInner {
            desktop_index: DesktopIconIndex::new(),
            cache: RefCell::new(IconCache::new(256)),
            inflight: RefCell::new(HashMap::new()),
            missing_names: RefCell::new(MissingIconCache::new(512)),
            worker,
        });
        let update_target = Rc::downgrade(&inner);
        glib::MainContext::default().spawn_local(async move {
            while let Ok(update) = update_rx.recv().await {
                // GTK objects are updated only from the owning main context
                let Some(inner) = update_target.upgrade() else {
                    break;
                };
                inner.handle_update(update);
            }
        });

        Self { inner }
    }

    pub fn apply_badge(
        &self,
        image: &gtk::Image,
        notification: &NotificationView,
        size: i32,
        scale: i32,
    ) {
        // Header badges deliberately exclude caller-controlled content image data and paths
        self.inner.apply_badge(image, notification, size, scale);
    }

    pub fn apply_sender_visual(&self, image: &gtk::Image, notification: &NotificationView) {
        self.inner.apply_sender_visual(image, notification);
    }

    pub fn apply_content_visual(&self, image: &gtk::Image, notification: &NotificationView) {
        self.inner.apply_content_visual(image, notification);
    }

    pub fn clear_missing_cache(&self) {
        // Theme reloads must retry names that were previously unavailable
        self.inner.clear_missing_cache();
    }
}

pub(super) struct IconResolverInner {
    pub(super) desktop_index: DesktopIconIndex,
    pub(super) cache: RefCell<IconCache>,
    pub(super) inflight: RefCell<HashMap<IconKey, Vec<glib::WeakRef<gtk::Image>>>>,
    pub(super) missing_names: RefCell<MissingIconCache>,
    pub(super) worker: IconWorker,
}

#[cfg(test)]
#[path = "tests/resolver.rs"]
mod tests;
