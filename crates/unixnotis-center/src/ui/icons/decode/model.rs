//! Values exchanged between icon decode workers and the GTK thread

use super::super::cache::IconKey;

pub(in crate::ui::icons) struct IconUpdate {
    pub(in crate::ui::icons) key: IconKey,
    pub(in crate::ui::icons) result: IconResult,
}

// Submission errors indicate overload or shutdown so callers can recover without blocking GTK
pub(in crate::ui::icons) enum IconSubmitError {
    Full,
    Closed,
}

impl IconSubmitError {
    pub(in crate::ui::icons) const fn reason(&self) -> &'static str {
        match self {
            Self::Full => "icon decode queue full (drop-newest)",
            Self::Closed => "icon decode queue closed",
        }
    }
}

#[derive(Debug)]
pub(in crate::ui::icons) enum IconResult {
    Raster(RasterImage),
    Failed(String),
}

#[derive(Debug)]
pub(in crate::ui::icons) struct RasterImage {
    pub(super) bytes: Vec<u8>,
    pub(super) width: i32,
    pub(super) height: i32,
    pub(super) stride: i32,
    // resvg returns premultiplied pixels while image decoders return straight alpha
    pub(super) premultiplied_alpha: bool,
}
