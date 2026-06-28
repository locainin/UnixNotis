use std::fs;
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

impl ServiceArtifact {
    pub(super) fn file(path: PathBuf, contents: String) -> Self {
        // File artifacts are the simplest manager-owned shape, used by systemd and dinit
        Self {
            path,
            kind: ServiceArtifactKind::File,
            contents: Some(contents),
            mode: None,
        }
    }

    pub fn is_present_safely(&self) -> bool {
        // State checks must match writer/remover ownership rules, not raw path existence
        match &self.kind {
            ServiceArtifactKind::File | ServiceArtifactKind::ExecutableFile => {
                // A symlink at a file path is never counted as installed
                path_is_regular_file(&self.path)
            }
            ServiceArtifactKind::SharedFile { .. } => {
                // Shared files are safe only when the existing bytes match the backend contract
                path_is_regular_file(&self.path)
                    && self
                        .contents
                        .as_ref()
                        .is_some_and(|expected| file_contents_match(&self.path, expected))
            }
            ServiceArtifactKind::Directory => path_is_directory(&self.path),
            ServiceArtifactKind::ManagedDirectory => {
                // Directory backends need the marker before state can call them installer-owned
                path_is_directory(&self.path)
                    && managed_directory_marker_is_valid(&managed_directory_marker(&self.path))
            }
            ServiceArtifactKind::Symlink { target } => fs::read_link(&self.path)
                // Symlink state is exact because enablement can depend on the stored target
                .map(|actual| actual == *target)
                .unwrap_or(false),
        }
    }

    pub fn exists_at_path_but_not_safely(&self) -> bool {
        // Unsafe paths are real filesystem entries that do not match the expected artifact shape
        // Reporting them separately avoids making symlinks or foreign directories look absent
        path_exists_without_following(&self.path) && !self.is_present_safely()
    }
}

fn path_is_regular_file(path: &Path) -> bool {
    fs::symlink_metadata(path)
        .map(|metadata| metadata.file_type().is_file())
        .unwrap_or(false)
}

fn path_is_directory(path: &Path) -> bool {
    fs::symlink_metadata(path)
        .map(|metadata| metadata.file_type().is_dir())
        .unwrap_or(false)
}

fn path_exists_without_following(path: &Path) -> bool {
    // symlink_metadata checks the artifact path itself, which is what safety diagnostics need
    fs::symlink_metadata(path).is_ok()
}

fn file_contents_match(path: &Path, expected: &str) -> bool {
    // Shared setup files use exact tiny contents, such as s6 bundle type declarations
    fs::read_to_string(path)
        .map(|contents| contents == expected)
        .unwrap_or(false)
}

pub fn managed_directory_marker(path: &Path) -> PathBuf {
    // Keep marker placement centralized so writer, remover, and state checks agree
    path.join(MANAGED_DIRECTORY_MARKER)
}

pub fn managed_directory_marker_is_valid(path: &Path) -> bool {
    let Ok(metadata) = fs::symlink_metadata(path) else {
        return false;
    };
    // A marker symlink is not ownership proof because it can point outside the service dir
    if metadata.file_type().is_symlink() || !metadata.file_type().is_file() {
        return false;
    }
    fs::read_to_string(path)
        .map(|contents| contents == MANAGED_DIRECTORY_MARKER_CONTENTS)
        .unwrap_or(false)
}
