//! Exact legacy stock helpers that can be upgraded without replacing user edits

use std::path::Path;

struct LegacyScript {
    relative_path: &'static str,
    contents: &'static [u8],
}

const LEGACY_SCRIPTS: &[LegacyScript] = &[
    LegacyScript {
        relative_path: "scripts/unixnotis-blue-light-lib",
        contents: include_bytes!("../../../../assets/scripts/legacy/unixnotis-blue-light-lib-v1"),
    },
    LegacyScript {
        relative_path: "scripts/unixnotis-blue-light-on",
        contents: include_bytes!("../../../../assets/scripts/legacy/unixnotis-blue-light-on-v1"),
    },
];

pub(super) fn is_legacy_stock_script(path: &Path, relative_path: &str) -> bool {
    let Some(legacy) = LEGACY_SCRIPTS
        .iter()
        .find(|legacy| legacy.relative_path == relative_path)
    else {
        return false;
    };

    // A metadata length check avoids reading an unrelated large user file
    let Ok(metadata) = path.symlink_metadata() else {
        return false;
    };
    if !metadata.file_type().is_file() || metadata.len() != legacy.contents.len() as u64 {
        return false;
    }

    // Exact bytes make the migration safe for every customized variant
    std::fs::read(path).is_ok_and(|contents| contents == legacy.contents)
}
