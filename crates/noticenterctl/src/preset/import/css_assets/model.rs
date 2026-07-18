//! Internal records for imported CSS asset materialization

use std::path::PathBuf;

#[derive(Debug)]
pub(super) enum ImportedCssReference {
    // Relative paths resolve to an included regular bundle file
    Bundled(PathBuf),
    // Data URLs are decoded before any native image library can inspect them
    Data {
        path_hint: PathBuf,
        contents: Vec<u8>,
    },
    // Expert-approved host-local or remote references remain byte-for-byte intact
    External,
}
