//! Standards-based classification for CSS file URLs

use std::os::unix::ffi::OsStrExt;
use std::path::PathBuf;

use url::Url;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileUrlClassification {
    Local(PathBuf),
    NonLocalAuthority,
    Malformed,
    NotFileUrl,
}

pub fn classify_file_url(value: &str) -> FileUrlClassification {
    let Some(prefix) = value.get(..5) else {
        return FileUrlClassification::NotFileUrl;
    };
    if !prefix.eq_ignore_ascii_case("file:") {
        return FileUrlClassification::NotFileUrl;
    }

    // Reject invalid escapes before URL parsing can preserve them as literal percent bytes
    if !has_valid_percent_encoding(value.as_bytes()) {
        return FileUrlClassification::Malformed;
    }
    let Ok(url) = Url::parse(value) else {
        return FileUrlClassification::Malformed;
    };
    if url.query().is_some() || url.fragment().is_some() || !url.username().is_empty() {
        return FileUrlClassification::Malformed;
    }
    if url
        .host_str()
        .is_some_and(|host| !host.eq_ignore_ascii_case("localhost"))
    {
        return FileUrlClassification::NonLocalAuthority;
    }

    let Ok(path) = url.to_file_path() else {
        return FileUrlClassification::Malformed;
    };
    if !path.is_absolute() || path.as_os_str().as_bytes().contains(&0) {
        return FileUrlClassification::Malformed;
    }
    FileUrlClassification::Local(path)
}

pub(super) fn has_valid_percent_encoding(bytes: &[u8]) -> bool {
    let mut index = 0usize;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            let Some(encoded) = bytes.get(index + 1..index + 3) else {
                return false;
            };
            if !encoded.iter().all(u8::is_ascii_hexdigit) {
                return false;
            }
            index += 3;
        } else {
            index += 1;
        }
    }
    true
}

#[cfg(test)]
#[path = "tests/file_url.rs"]
mod tests;
