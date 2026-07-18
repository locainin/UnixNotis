//! Descriptor-pinned icon loading and bounded worker decoding

mod file;
mod model;
mod pipeline;
mod raster;
mod svg;
mod texture;
mod worker;

pub(super) use model::{IconResult, IconUpdate};
pub(super) use texture::texture_from_raster;
pub(super) use worker::IconWorker;
