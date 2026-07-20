//! Safe filesystem writes shared by configuration provisioning paths

use std::path::Path;

use crate::filesystem::write_file_if_missing;

use super::ConfigError;

pub(super) fn write_if_missing(path: &Path, contents: &str) -> Result<(), ConfigError> {
    write_file_if_missing(path, contents.as_bytes(), 0o644)
        .map(|_created| ())
        .map_err(|err| ConfigError::ReadFailed(err.to_string()))
}
