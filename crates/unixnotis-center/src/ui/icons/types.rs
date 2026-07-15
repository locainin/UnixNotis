//! Internal icon resolution and decode request types

use std::path::PathBuf;
use std::rc::Rc;

use super::cache::{CachedPaintable, IconKey};
use super::decode::IconDecodeMode;

pub(super) enum IconResolution {
    Ready {
        key: IconKey,
        paintable: Rc<CachedPaintable>,
    },
    Async {
        request: IconDecodeRequest,
    },
}

pub(super) struct IconDecodeRequest {
    pub(super) key: IconKey,
    pub(super) path: PathBuf,
    pub(super) size: i32,
    pub(super) scale: i32,
    pub(super) mode: IconDecodeMode,
}

#[cfg(test)]
#[path = "tests/types.rs"]
mod tests;
