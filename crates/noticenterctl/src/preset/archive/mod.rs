//! Archive reads, writes, limits, and stored file metadata

mod budget;
mod model;
mod modes;
mod preflight;
mod read;
#[cfg(test)]
mod tests;
mod write;

pub(super) use model::{BundleArchive, BundleFile};
pub(super) use read::{
    read_bundle, MAX_PRESET_FILE_BYTES, MAX_PRESET_PAYLOAD_FILES, MAX_PRESET_TOTAL_PAYLOAD_BYTES,
};
pub(super) use write::write_bundle;
