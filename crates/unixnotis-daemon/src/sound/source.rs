//! Descriptor-pinned sound inputs

use std::fs::File;
use std::os::fd::AsRawFd;
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Debug, Clone)]
pub(super) struct SoundFile {
    // The original path is retained only for diagnostics and policy checks
    path: PathBuf,
    // The open file pins the validated object until the playback child exits
    file: Arc<File>,
}

impl SoundFile {
    pub(super) fn new(path: PathBuf, file: File) -> Self {
        Self {
            path,
            file: Arc::new(file),
        }
    }

    pub(super) fn path(&self) -> &Path {
        &self.path
    }

    pub(super) fn playback_path(&self) -> PathBuf {
        // The child opens the daemon's retained descriptor instead of resolving the source again
        PathBuf::from("/proc")
            .join(std::process::id().to_string())
            .join("fd")
            .join(self.file.as_raw_fd().to_string())
    }

    pub(super) fn keepalive(&self) -> Arc<File> {
        self.file.clone()
    }
}

#[derive(Debug, Clone)]
pub(super) enum SoundSource {
    Name(String),
    File(SoundFile),
}

#[cfg(test)]
#[path = "tests/source.rs"]
mod tests;
