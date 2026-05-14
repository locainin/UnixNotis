use std::cell::RefCell;
use std::io::Cursor;
use std::path::Path;
use std::rc::Rc;
use std::time::Duration;

use gtk::prelude::*;
use gtk::{gdk, gio, glib};
use image::ImageReader;
use tracing::debug;

use crate::media::MediaArtSource;

// Small budgets keep artwork from becoming a memory or latency sink
const MEDIA_ART_TIMEOUT: Duration = Duration::from_secs(3);
const MEDIA_ART_CHUNK_BYTES: usize = 64 * 1024;
const MAX_LOCAL_ART_BYTES: u64 = 8 * 1024 * 1024;
const MAX_REMOTE_ART_BYTES: usize = 2 * 1024 * 1024;
const MAX_MEDIA_ART_DIMENSION: u32 = 2048;
const MAX_MEDIA_ART_PIXELS: u32 = 4_194_304;

#[derive(Debug, Default)]
pub(super) struct MediaArtState {
    // Tracks the art key that is actually visible right now
    displayed_key: Option<String>,
    // Tracks the latest key still loading in the background
    pending_key: Option<String>,
    // Bumps every time a fresh request takes ownership of the slot
    pending_gen: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum MediaArtCompletion {
    Ignore,
    Apply,
    Clear,
}

pub(super) fn apply_media_art(
    picture: &gtk::Picture,
    art_state: &Rc<RefCell<MediaArtState>>,
    source: Option<&MediaArtSource>,
) {
    let next_key = source.map(MediaArtSource::stable_key);

    {
        let mut art_state = art_state.borrow_mut();
        // If the current picture already matches the requested art, keep it and
        // cancel any stale in-flight replacement
        if art_state.keep_displayed_if_current(&next_key) {
            return;
        }
        // Matching pending keys mean the same replacement is already loading
        if art_state.pending_key_matches(&next_key) {
            return;
        }
    }

    let Some(source) = source.cloned() else {
        // A real None means the player no longer has art, so clear right away
        art_state.borrow_mut().clear_displayed_now();
        show_empty_picture(picture);
        return;
    };

    // Invalid local files fail fast before any async work is queued
    if matches!(&source, MediaArtSource::LocalFile(path) if !local_art_file_allowed(path)) {
        // Bad sources should not poison later retries for the same key
        art_state.borrow_mut().clear_displayed_now();
        show_empty_picture(picture);
        return;
    }

    let request_gen = art_state.borrow_mut().begin_request(next_key.clone());
    let picture_weak = picture.downgrade();
    let art_state = art_state.clone();
    glib::MainContext::default().spawn_local(async move {
        let texture = load_art_texture(&source).await;
        let Some(picture) = picture_weak.upgrade() else {
            return;
        };

        // Completion is ignored when a newer request already claimed the slot
        let completion =
            art_state
                .borrow_mut()
                .finish_request(request_gen, next_key.clone(), texture.is_some());

        match (completion, texture) {
            (MediaArtCompletion::Apply, Some(texture)) => show_loaded_texture(&picture, &texture),
            (MediaArtCompletion::Clear, _) => show_empty_picture(&picture),
            _ => {}
        }
    });
}

fn clear_picture(picture: &gtk::Picture) {
    // Clearing both file and paintable avoids mixing old and new loading paths
    picture.set_file(None::<&gio::File>);
    picture.set_paintable(None::<&gdk::Texture>);
}

fn show_loaded_texture(picture: &gtk::Picture, texture: &gdk::Texture) {
    // Keep the previous art on screen until the replacement is fully ready
    picture.set_file(None::<&gio::File>);
    picture.set_paintable(Some(texture));
    picture.remove_css_class("empty");
}

fn show_empty_picture(picture: &gtk::Picture) {
    // Empty state is only shown when there is truly no art or the replacement failed
    clear_picture(picture);
    picture.add_css_class("empty");
}

fn local_art_file_allowed(path: &Path) -> bool {
    let Ok(metadata) = std::fs::metadata(path) else {
        return false;
    };
    // Only regular files are allowed here to avoid device and fifo edge cases
    metadata.is_file() && metadata.len() <= MAX_LOCAL_ART_BYTES
}

async fn load_art_texture(source: &MediaArtSource) -> Option<gdk::Texture> {
    // The timeout drops the GIO future which cancels the in-flight read
    let bytes = glib::future_with_timeout(MEDIA_ART_TIMEOUT, load_art_bytes(source))
        .await
        .ok()??;

    // Decode runs on the Gio worker pool so track skips do not block GTK
    let raster = gio::spawn_blocking(move || decode_art_raster(bytes))
        .await
        .ok()??;
    Some(texture_from_raster(raster))
}

async fn load_art_bytes(source: &MediaArtSource) -> Option<Vec<u8>> {
    match source {
        MediaArtSource::LocalFile(path) => {
            // Native paths are safer to load through a local file handle
            let file = gio::File::for_path(path);
            read_file_bytes_limited(&file, MAX_LOCAL_ART_BYTES as usize).await
        }
        MediaArtSource::RemoteHttps(url) => {
            // Remote art still goes through the same byte cap as local art
            let file = gio::File::for_uri(url.as_str());
            read_file_bytes_limited(&file, MAX_REMOTE_ART_BYTES).await
        }
    }
}

async fn read_file_bytes_limited(file: &gio::File, max_bytes: usize) -> Option<Vec<u8>> {
    let stream = file.read_future(glib::Priority::default()).await.ok()?;
    let mut out = Vec::new();
    loop {
        let chunk = stream
            .read_bytes_future(MEDIA_ART_CHUNK_BYTES, glib::Priority::default())
            .await
            .ok()?;
        if chunk.is_empty() {
            break;
        }
        let bytes = chunk.as_ref();
        if out.len().saturating_add(bytes.len()) > max_bytes {
            debug!(
                max_bytes,
                "media artwork rejected because it exceeded the byte cap"
            );
            return None;
        }
        out.extend_from_slice(bytes);
    }
    if out.is_empty() {
        return None;
    }
    Some(out)
}

fn decode_art_raster(bytes: Vec<u8>) -> Option<DecodedArt> {
    let (width, height) = art_dimensions_from_bytes(&bytes)?;
    if !art_dimensions_allowed(width, height) {
        debug!("media artwork rejected because dimensions were too large");
        return None;
    }

    let rgba = image::load_from_memory(&bytes).ok()?.to_rgba8();
    let width = rgba.width();
    let height = rgba.height();
    let width = i32::try_from(width).ok()?;
    let height = i32::try_from(height).ok()?;
    let stride = width.checked_mul(4)?;

    Some(DecodedArt {
        bytes: rgba.into_raw(),
        width,
        height,
        stride,
    })
}

fn art_dimensions_from_bytes(bytes: &[u8]) -> Option<(u32, u32)> {
    let cursor = Cursor::new(bytes);
    let reader = ImageReader::new(cursor).with_guessed_format().ok()?;
    // Dimension checks happen before the full decode work runs
    reader.into_dimensions().ok()
}

fn art_dimensions_allowed(width: u32, height: u32) -> bool {
    if width == 0 || height == 0 {
        return false;
    }
    if width > MAX_MEDIA_ART_DIMENSION || height > MAX_MEDIA_ART_DIMENSION {
        return false;
    }
    width.saturating_mul(height) <= MAX_MEDIA_ART_PIXELS
}

fn texture_from_raster(raster: DecodedArt) -> gdk::Texture {
    // MemoryTexture lets GTK keep the decoded RGBA bytes without re-decoding the source
    let bytes = glib::Bytes::from_owned(raster.bytes);
    gdk::MemoryTexture::new(
        raster.width,
        raster.height,
        gdk::MemoryFormat::R8g8b8a8,
        &bytes,
        raster.stride as usize,
    )
    .upcast::<gdk::Texture>()
}

#[derive(Debug)]
struct DecodedArt {
    bytes: Vec<u8>,
    width: i32,
    height: i32,
    stride: i32,
}

impl MediaArtState {
    fn keep_displayed_if_current(&mut self, next_key: &Option<String>) -> bool {
        if self.displayed_key != *next_key {
            return false;
        }
        // A revert back to the currently shown art should invalidate stale in-flight work
        if self.pending_key.is_some() {
            self.pending_gen = self.pending_gen.wrapping_add(1);
            self.pending_key = None;
        }
        true
    }

    fn pending_key_matches(&self, next_key: &Option<String>) -> bool {
        self.pending_key == *next_key
    }

    fn begin_request(&mut self, next_key: Option<String>) -> u64 {
        self.pending_gen = self.pending_gen.wrapping_add(1);
        self.pending_key = next_key;
        self.pending_gen
    }

    fn clear_displayed_now(&mut self) {
        self.pending_gen = self.pending_gen.wrapping_add(1);
        self.pending_key = None;
        self.displayed_key = None;
    }

    fn finish_request(
        &mut self,
        request_gen: u64,
        request_key: Option<String>,
        success: bool,
    ) -> MediaArtCompletion {
        if self.pending_gen != request_gen {
            return MediaArtCompletion::Ignore;
        }

        self.pending_key = None;
        if success {
            self.displayed_key = request_key;
            return MediaArtCompletion::Apply;
        }

        // Failed replacements clear the slot, but they do not claim the key
        self.displayed_key = None;
        MediaArtCompletion::Clear
    }
}

#[cfg(test)]
#[path = "tests/media_art.rs"]
mod tests;
