//! Icon-specific limits over the shared single-open local-file reader

use std::path::Path;

use crate::ui::local_file::read_regular_file;

pub(super) const MAX_ICON_BYTES: u64 = 16 * 1_024 * 1_024;

pub(super) fn read_icon_file(path: &Path) -> Result<Vec<u8>, String> {
    read_regular_file(path, MAX_ICON_BYTES).map_err(|error| error.to_string())
}
