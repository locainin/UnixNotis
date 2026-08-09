use std::fs;
use std::io;
use std::path::{Path, PathBuf};

pub const MANAGED_DIRECTORY_MARKER: &str = ".unixnotis-managed";
pub const MANAGED_DIRECTORY_MARKER_CONTENTS: &str = "unixnotis\n";

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ServiceArtifactKind {
    // Plain backend-owned file, such as a user service definition
    File,
    // Shared setup file seeded only when missing and removed only while still byte-for-byte owned
    SharedFile { created_marker: Option<PathBuf> },
    // Script-style managers need an explicit executable bit on generated run files
    ExecutableFile,
    // Supervision trees can need a service directory rather than a single file
    Directory,
    // Recursively removed service directories need a marker proving installer ownership
    ManagedDirectory,
    // Activation trees often reference service directories through manager-owned links
    Symlink { target: PathBuf },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ServiceArtifact {
    // Exact path the installer owns for this artifact
    pub path: PathBuf,
    // Filesystem shape controls write, chmod, and cleanup behavior
    pub kind: ServiceArtifactKind,
    // Directories and symlinks intentionally have no file body
    pub contents: Option<String>,
    // Executable modes are applied explicitly by the installer on Unix
    pub mode: Option<u32>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ServiceArtifactState {
    Missing,
    Expected,
    UnexpectedObject,
}

impl ServiceArtifact {
    pub(in crate::service_manager) const fn file(path: PathBuf, contents: String) -> Self {
        // File artifacts are the simplest manager-owned shape, used by systemd and dinit
        Self {
            path,
            kind: ServiceArtifactKind::File,
            contents: Some(contents),
            mode: None,
        }
    }

    pub fn is_present_safely(&self) -> bool {
        // Compatibility callers need a boolean while conflict scans retain inspection errors
        self.inspect()
            .is_ok_and(|state| state == ServiceArtifactState::Expected)
    }

    pub fn exists_at_path_but_not_safely(&self) -> bool {
        self.inspect()
            .is_ok_and(|state| state == ServiceArtifactState::UnexpectedObject)
    }

    pub fn inspect(&self) -> io::Result<ServiceArtifactState> {
        let metadata = match fs::symlink_metadata(&self.path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Ok(ServiceArtifactState::Missing)
            }
            Err(error) => return Err(error),
        };

        let expected = match &self.kind {
            ServiceArtifactKind::File | ServiceArtifactKind::ExecutableFile => {
                metadata.file_type().is_file()
            }
            ServiceArtifactKind::SharedFile { .. } => {
                if metadata.file_type().is_file() {
                    let Some(expected) = self.contents.as_ref() else {
                        return Ok(ServiceArtifactState::UnexpectedObject);
                    };
                    fs::read_to_string(&self.path)? == *expected
                } else {
                    false
                }
            }
            ServiceArtifactKind::Directory => metadata.file_type().is_dir(),
            ServiceArtifactKind::ManagedDirectory => {
                metadata.file_type().is_dir() && inspect_managed_marker(&self.path)?
            }
            ServiceArtifactKind::Symlink { target } => {
                metadata.file_type().is_symlink() && fs::read_link(&self.path)? == *target
            }
        };
        Ok(if expected {
            ServiceArtifactState::Expected
        } else {
            ServiceArtifactState::UnexpectedObject
        })
    }
}

fn inspect_managed_marker(directory: &Path) -> io::Result<bool> {
    let marker = managed_directory_marker(directory);
    let metadata = match fs::symlink_metadata(&marker) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error),
    };
    if !metadata.file_type().is_file() {
        return Ok(false);
    }
    Ok(fs::read_to_string(marker)? == MANAGED_DIRECTORY_MARKER_CONTENTS)
}

pub fn managed_directory_marker(path: &Path) -> PathBuf {
    // Keep marker placement centralized so writer, remover, and state checks agree
    path.join(MANAGED_DIRECTORY_MARKER)
}
