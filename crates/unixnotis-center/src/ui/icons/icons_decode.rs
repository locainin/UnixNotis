//! Background decoding for raster icons.
//!
//! Offloads image decoding and resizing to worker threads.

use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::thread;

use crossbeam_channel as channel;
use fast_image_resize as fir;
use gtk::gdk;
use gtk::gdk::Texture;
use gtk::glib;
use gtk::prelude::*;

use super::icons_cache::IconKey;

// Prevent unbounded reads from untrusted icon paths.
const MAX_ICON_BYTES: u64 = 16 * 1024 * 1024;
const MAX_ICON_DIMENSION: u32 = 2048;

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
        // Unbounded job queue; UI thread submits decode work, workers consume.
        let (sender, receiver) = channel::unbounded::<IconJob>();

        // Keep worker count small (<=2) because decode is CPU-heavy and we don't want to starve GTK.
        // available_parallelism() may fail in constrained environments, so default to 1.
        let worker_count = thread::available_parallelism()
            .map(|count| count.get().min(2))
            .unwrap_or(1);

        for _ in 0..worker_count {
            let receiver = receiver.clone();
            let update_tx = update_tx.clone();

            thread::spawn(move || {
                // Blocking worker loop: wait for decode jobs, run decode, report back to UI via update_tx.
                for job in receiver.iter() {
                    let IconJob::Decode {
                        key,
                        path,
                        size,
                        scale,
                    } = job;

                    // Decode off-thread; GTK objects should be created/applied on the main loop later.
                    let result = decode_raster(&path, size, scale);

                    // send_blocking is fine here (worker thread), avoids busy looping if UI is momentarily slow.
                    let _ = update_tx.send_blocking(IconUpdate { key, result });
                }
            });
        }

        Self { sender }
    }

    pub(super) fn submit_decode(&self, key: IconKey, path: PathBuf, size: i32, scale: i32) {
        // Best-effort enqueue; if the worker is shut down, dropping the job is acceptable.
        let _ = self.sender.send(IconJob::Decode {
            key,
            path,
            size,
            scale,
        });
    }
}

fn decode_raster(path: &Path, size: i32, scale: i32) -> IconResult {
    let metadata = match std::fs::metadata(path) {
        Ok(metadata) => metadata,
        Err(err) => return IconResult::Failed(err.to_string()),
    };
    if !metadata.is_file() {
        return IconResult::Failed("icon path is not a regular file".to_string());
    }
    if metadata.len() > MAX_ICON_BYTES {
        return IconResult::Failed(format!("icon file too large ({} bytes)", metadata.len()));
    }

    // Read the file into memory with a hard cap to avoid unbounded allocations.
    let file = match File::open(path) {
        Ok(file) => file,
        Err(err) => return IconResult::Failed(err.to_string()),
    };
    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    let mut limited = file.take(MAX_ICON_BYTES + 1);
    if let Err(err) = limited.read_to_end(&mut bytes) {
        return IconResult::Failed(err.to_string());
    }
    if bytes.len() as u64 > MAX_ICON_BYTES {
        return IconResult::Failed("icon file too large".to_string());
    }

    // Decode the image from the raw bytes. load_from_memory auto-detects the format.
    let image = match image::load_from_memory(&bytes) {
        Ok(image) => image,
        Err(err) => return IconResult::Failed(err.to_string()),
    };

    // Compute target pixel size. size is logical units; scale accounts for output scale (e.g. 2x).
    // max(1) prevents zero/negative values from producing nonsense.
    let size = i64::from(size.max(1));
    let scale = i64::from(scale.max(1));
    let target = size
        .saturating_mul(scale)
        .clamp(1, MAX_ICON_DIMENSION as i64) as u32;

    // Convert to RGBA8 so the SIMD resizer works on a stable pixel layout.
    let rgba = image.to_rgba8();
    let width = rgba.width();
    let height = rgba.height();
    if width > i32::MAX as u32 || height > i32::MAX as u32 {
        return IconResult::Failed("decoded icon exceeds supported dimensions".to_string());
    }
    let src =
        match fir::images::Image::from_vec_u8(width, height, rgba.into_raw(), fir::PixelType::U8x4)
        {
            Ok(src) => src,
            Err(err) => return IconResult::Failed(err.to_string()),
        };
    let mut dst = fir::images::Image::new(target, target, fir::PixelType::U8x4);
    let options = fir::ResizeOptions::new()
        .resize_alg(fir::ResizeAlg::Convolution(fir::FilterType::CatmullRom));
    let mut resizer = fir::Resizer::new();
    if let Err(err) = resizer.resize(&src, &mut dst, Some(&options)) {
        return IconResult::Failed(err.to_string());
    }

    let width = target as i32;
    let height = target as i32;

    // Bytes per row for RGBA8. saturating_mul avoids overflow if width is unexpectedly large.
    let stride = width.saturating_mul(4);

    // into_vec consumes the resize buffer and returns the owned RGBA bytes (no extra copy).
    IconResult::Raster(RasterImage {
        bytes: dst.into_vec(),
        width,
        height,
        stride,
    })
}

pub(super) fn texture_from_raster(image: &RasterImage) -> Texture {
    // Wrap the Vec<u8> as glib::Bytes so GTK can reference it efficiently.
    // MemoryTexture copies/uses the bytes per GTK expectations; stride must match row size.
    let bytes = glib::Bytes::from(&image.bytes);

    gdk::MemoryTexture::new(
        image.width,                 // pixel width
        image.height,                // pixel height
        gdk::MemoryFormat::R8g8b8a8, // matches RGBA8 layout from decode_raster()
        &bytes,                      // backing storage
        image.stride as usize,       // bytes per row
    )
    .upcast::<Texture>()
}
