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

use super::cache::IconKey;

// Prevent unbounded reads from untrusted icon paths.
const MAX_ICON_BYTES: u64 = 16 * 1024 * 1024;
const MAX_ICON_DIMENSION: u32 = 2048;
const ICON_DECODE_QUEUE_CAPACITY: usize = 128;

enum IconDecodeDropPolicy {
    DropNewest,
}

// Bound decode queue growth to protect against bursts of unique icon paths.
const ICON_DECODE_DROP_POLICY: IconDecodeDropPolicy = IconDecodeDropPolicy::DropNewest;

pub(super) struct IconWorker {
    sender: channel::Sender<IconJob>,
    // Test-only guard keeps the update channel open when no workers are spawned.
    #[cfg(test)]
    #[allow(dead_code)]
    update_tx_guard: async_channel::Sender<IconUpdate>,
    // Test-only receiver guard keeps the channel open when no workers are spawned.
    #[cfg(test)]
    #[allow(dead_code)]
    receiver_guard: channel::Receiver<IconJob>,
}

pub(super) struct IconUpdate {
    pub(super) key: IconKey,
    pub(super) result: IconResult,
}

// Submission errors indicate overload or shutdown; callers decide how to recover.
pub(super) enum IconSubmitError {
    Full,
    Closed,
}

impl IconSubmitError {
    pub(super) fn reason(&self) -> &'static str {
        match self {
            IconSubmitError::Full => match ICON_DECODE_DROP_POLICY {
                IconDecodeDropPolicy::DropNewest => "icon decode queue full (drop-newest)",
            },
            IconSubmitError::Closed => "icon decode queue closed",
        }
    }
}

#[derive(Debug)]
pub(super) enum IconResult {
    Raster(RasterImage),
    Bytes(Vec<u8>),
    Failed(String),
}

#[derive(Debug)]
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
        mode: IconDecodeMode,
    },
}

#[derive(Copy, Clone, Debug)]
pub(super) enum IconDecodeMode {
    Raster,
    Bytes,
}

impl IconWorker {
    pub(super) fn new(update_tx: async_channel::Sender<IconUpdate>) -> Self {
        Self::new_with_capacity(update_tx, ICON_DECODE_QUEUE_CAPACITY, true)
    }

    fn new_with_capacity(
        update_tx: async_channel::Sender<IconUpdate>,
        capacity: usize,
        spawn_workers: bool,
    ) -> Self {
        // Bounded job queue; drop policy applies when overload occurs.
        let (sender, receiver) = channel::bounded::<IconJob>(capacity);
        #[cfg(test)]
        let receiver_guard = receiver.clone();
        #[cfg(test)]
        let update_tx_guard = update_tx.clone();

        // Keep worker count small (<=2) because decode is CPU-heavy and we don't want to starve GTK.
        // available_parallelism() may fail in constrained environments, so default to 1.
        let worker_count = thread::available_parallelism()
            .map(|count| count.get().min(2))
            .unwrap_or(1);

        if spawn_workers {
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
                            mode,
                        } = job;

                        // Decode off-thread; GTK objects should be created/applied on the main loop later.
                        let result = match mode {
                            IconDecodeMode::Raster => decode_raster(&path, size, scale),
                            // Bytes mode keeps file I/O off the GTK thread for formats that
                            // are still decoded on the main loop (e.g., SVG via GDK).
                            IconDecodeMode::Bytes => load_bytes(&path),
                        };

                        // send_blocking is fine here (worker thread), avoids busy looping if UI is momentarily slow.
                        let _ = update_tx.send_blocking(IconUpdate { key, result });
                    }
                });
            }
        }

        Self {
            sender,
            #[cfg(test)]
            update_tx_guard,
            #[cfg(test)]
            receiver_guard,
        }
    }

    pub(super) fn submit_decode(
        &self,
        key: IconKey,
        path: PathBuf,
        size: i32,
        scale: i32,
        mode: IconDecodeMode,
    ) -> Result<(), IconSubmitError> {
        // Non-blocking submit; overload handling is delegated to the caller.
        let job = IconJob::Decode {
            // Move the owned key straight into the job so the caller does not pay
            // for an extra deep clone before the worker even starts
            key,
            path,
            size,
            scale,
            mode,
        };
        match self.sender.try_send(job) {
            Ok(()) => Ok(()),
            Err(channel::TrySendError::Full(_job)) => Err(IconSubmitError::Full),
            Err(channel::TrySendError::Disconnected(_)) => Err(IconSubmitError::Closed),
        }
    }
}

#[cfg(test)]
impl IconWorker {
    fn new_for_tests(update_tx: async_channel::Sender<IconUpdate>, capacity: usize) -> Self {
        Self::new_with_capacity(update_tx, capacity, false)
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
    // Skip the resize path when the source already matches the target size.
    if width == target && height == target {
        let width = width as i32;
        let height = height as i32;
        let stride = width.saturating_mul(4);
        return IconResult::Raster(RasterImage {
            bytes: rgba.into_raw(),
            width,
            height,
            stride,
        });
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

fn load_bytes(path: &Path) -> IconResult {
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
    IconResult::Bytes(bytes)
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn icon_worker_queue_overflow_reports_failure() {
        let (update_tx, update_rx) = async_channel::bounded(2);
        let worker = IconWorker::new_for_tests(update_tx, 1);
        let key_a = IconKey::Path {
            path: "icon-a.png".to_string(),
            size: 16,
            scale: 1,
        };
        let key_b = IconKey::Path {
            path: "icon-b.png".to_string(),
            size: 16,
            scale: 1,
        };

        assert!(worker
            .submit_decode(
                key_a,
                PathBuf::from("icon-a.png"),
                16,
                1,
                IconDecodeMode::Raster,
            )
            .is_ok());
        let err = worker
            .submit_decode(
                key_b,
                PathBuf::from("icon-b.png"),
                16,
                1,
                IconDecodeMode::Raster,
            )
            .expect_err("queue should be full");

        assert!(matches!(err, IconSubmitError::Full));
        assert!(matches!(
            update_rx.try_recv(),
            Err(async_channel::TryRecvError::Empty)
        ));
    }

    #[test]
    fn icon_submit_error_reasons_are_stable() {
        assert_eq!(
            IconSubmitError::Full.reason(),
            "icon decode queue full (drop-newest)"
        );
        assert_eq!(IconSubmitError::Closed.reason(), "icon decode queue closed");
    }

    #[test]
    fn decode_raster_reports_missing_file() {
        let result = decode_raster(Path::new("missing-icon.png"), 16, 1);
        assert!(matches!(result, IconResult::Failed(_)));
    }

    #[test]
    fn load_bytes_reads_small_file() {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("unixnotis-icon-bytes-{stamp}.bin"));
        let payload = b"svg-bytes";
        fs::write(&path, payload).expect("write temp icon bytes");

        let result = load_bytes(&path);
        match result {
            IconResult::Bytes(bytes) => assert_eq!(bytes, payload),
            other => panic!("unexpected result: {other:?}"),
        }

        let _ = fs::remove_file(&path);
    }
}
