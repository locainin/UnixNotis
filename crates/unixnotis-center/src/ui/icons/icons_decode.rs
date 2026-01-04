//! Background decoding for raster icons.
//!
//! Offloads image decoding and resizing to worker threads.

use std::path::{Path, PathBuf};
use std::thread;

use crossbeam_channel as channel;
use gtk::gdk;
use gtk::glib;
use gtk::prelude::*;
use gtk::gdk::Texture;
use image::imageops::FilterType;

use super::icons_cache::IconKey;

pub(super) struct IconWorker {
    sender: channel::Sender<IconJob>,
}

pub(super) struct IconUpdate {
    pub(super) key: IconKey,
    pub(super) result: IconResult,
}

pub(super) enum IconResult {
    Raster(RasterImage),
    Failed(String),
}

pub(super) struct RasterImage {
    pub(super) bytes: Vec<u8>,
    pub(super) width: i32,
    pub(super) height: i32,
    pub(super) stride: i32,
}

enum IconJob {
    Decode {
        key: IconKey,
        path: PathBuf,
        size: i32,
        scale: i32,
    },
}

impl IconWorker {
    pub(super) fn new(update_tx: async_channel::Sender<IconUpdate>) -> Self {
        let (sender, receiver) = channel::unbounded::<IconJob>();
        let worker_count = thread::available_parallelism()
            .map(|count| count.get().min(2))
            .unwrap_or(1);
        for _ in 0..worker_count {
            let receiver = receiver.clone();
            let update_tx = update_tx.clone();
            thread::spawn(move || {
                for job in receiver.iter() {
                    let IconJob::Decode {
                        key,
                        path,
                        size,
                        scale,
                    } = job;
                    let result = decode_raster(&path, size, scale);
                    let _ = update_tx.send_blocking(IconUpdate { key, result });
                }
            });
        }
        Self { sender }
    }

    pub(super) fn submit_decode(&self, key: IconKey, path: PathBuf, size: i32, scale: i32) {
        let _ = self.sender.send(IconJob::Decode {
            key,
            path,
            size,
            scale,
        });
    }
}

fn decode_raster(path: &Path, size: i32, scale: i32) -> IconResult {
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(err) => return IconResult::Failed(err.to_string()),
    };
    let image = match image::load_from_memory(&bytes) {
        Ok(image) => image,
        Err(err) => return IconResult::Failed(err.to_string()),
    };
    let target = (size.max(1) * scale.max(1)) as u32;
    let resized = image.resize_exact(target, target, FilterType::CatmullRom);
    let rgba = resized.to_rgba8();
    let width = rgba.width() as i32;
    let height = rgba.height() as i32;
    let stride = width.saturating_mul(4);
    IconResult::Raster(RasterImage {
        bytes: rgba.into_raw(),
        width,
        height,
        stride,
    })
}

pub(super) fn texture_from_raster(image: &RasterImage) -> Texture {
    let bytes = glib::Bytes::from(&image.bytes);
    gdk::MemoryTexture::new(
        image.width,
        image.height,
        gdk::MemoryFormat::R8g8b8a8,
        &bytes,
        image.stride as usize,
    )
    .upcast::<Texture>()
}
