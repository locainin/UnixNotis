//! Asynchronous icon request and completion handling

use std::path::Path;

use gdk_pixbuf::Pixbuf;
use gio::MemoryInputStream;
use gtk::gdk;
use gtk::glib;
use gtk::prelude::*;
use tracing::debug;

use super::cache::{image_key_matches, CachedPaintable, IconKey};
use super::decode::{texture_from_raster, IconResult, IconUpdate};
use super::resolver::IconResolverInner;
use super::theme::resolve_path_texture;
use super::types::IconDecodeRequest;

impl IconResolverInner {
    pub(super) fn enqueue(&self, request: IconDecodeRequest, image: &gtk::Image) {
        let IconDecodeRequest {
            key,
            path,
            size,
            scale,
            mode,
        } = request;
        let mut inflight = self.inflight.borrow_mut();
        if let Some(waiters) = inflight.get_mut(&key) {
            // A matching request shares the same worker result
            waiters.push(image.downgrade());
            return;
        }
        inflight.insert(key.clone(), vec![image.downgrade()]);
        drop(inflight);

        if let Err(err) = self
            .worker
            .submit_decode(key.clone(), path, size, scale, mode)
        {
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
            IconResult::Bytes(bytes) => self.decode_bytes(&key, bytes),
            IconResult::Failed(err) => {
                debug!(?err, "icon decode failed");
                match &key {
                    IconKey::Path { path, .. } => resolve_path_texture(Path::new(path))
                        .map(|texture| self.cache.borrow_mut().insert(key.clone(), texture)),
                    _ => None,
                }
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

    fn decode_bytes(&self, key: &IconKey, bytes: Vec<u8>) -> Option<std::rc::Rc<CachedPaintable>> {
        let (size, scale) = key.size_and_scale();
        let target = decoded_target(size, scale);
        let bytes = glib::Bytes::from_owned(bytes);
        let stream = MemoryInputStream::from_bytes(&bytes);
        match Pixbuf::from_stream_at_scale(&stream, target, target, true, None::<&gio::Cancellable>)
        {
            Ok(pixbuf) => {
                let texture = gdk::Texture::for_pixbuf(&pixbuf);
                Some(
                    self.cache
                        .borrow_mut()
                        .insert(key.clone(), CachedPaintable::from_texture(texture)),
                )
            }
            Err(err) => {
                debug!(?err, "icon byte decode failed");
                None
            }
        }
    }
}

fn decoded_target(size: i32, scale: i32) -> i32 {
    size.max(1).saturating_mul(scale.max(1)).max(1)
}

#[cfg(test)]
#[path = "tests/updates.rs"]
mod tests;
