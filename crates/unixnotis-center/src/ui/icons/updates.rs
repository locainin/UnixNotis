//! Asynchronous icon request and completion handling

use gtk::prelude::*;
use tracing::debug;

use super::cache::{image_key_matches, CachedPaintable};
use super::decode::{texture_from_raster, IconResult, IconUpdate};
use super::resolver::IconResolverInner;
use super::types::IconDecodeRequest;

impl IconResolverInner {
    pub(super) fn enqueue(&self, request: IconDecodeRequest, image: &gtk::Image) {
        let IconDecodeRequest {
            key,
            path,
            size,
            scale,
        } = request;
        let mut inflight = self.inflight.borrow_mut();
        if let Some(waiters) = inflight.get_mut(&key) {
            // A matching request shares the same worker result
            waiters.push(image.downgrade());
            return;
        }
        inflight.insert(key.clone(), vec![image.downgrade()]);
        drop(inflight);

        if let Err(err) = self.worker.submit_decode(key.clone(), path, size, scale) {
            // Failed submissions clear inflight state through the normal update path
            self.handle_update(IconUpdate {
                key,
                result: IconResult::Failed(err.reason().to_string()),
            });
        }
    }

    pub(super) fn handle_update(&self, update: IconUpdate) {
        let IconUpdate { key, result } = update;
        let waiters = self.inflight.borrow_mut().remove(&key).unwrap_or_default();

        let paintable = match result {
            IconResult::Raster(image) => {
                let texture = texture_from_raster(&image);
                Some(
                    self.cache
                        .borrow_mut()
                        .insert(key.clone(), CachedPaintable::from_texture(texture)),
                )
            }
            IconResult::Failed(err) => {
                debug!(?err, "icon decode failed");
                None
            }
        };

        let Some(paintable) = paintable else {
            return;
        };
        for waiter in waiters {
            let Some(image) = waiter.upgrade() else {
                continue;
            };
            // Stale completions cannot overwrite a newer icon request
            if image_key_matches(&image, &key) {
                image.set_paintable(Some(paintable.paintable()));
                image.set_visible(true);
            }
        }
    }
}
