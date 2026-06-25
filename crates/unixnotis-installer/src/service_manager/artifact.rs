use std::path::PathBuf;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ServiceArtifactKind {
    // Plain backend-owned file, such as a user service definition
    File,
    // Script-style managers need an explicit executable bit on generated run files
    ExecutableFile,
    // Supervision trees can need a service directory rather than a single file
    Directory,
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
        Self {
            path,
            kind: ServiceArtifactKind::File,
            contents: Some(contents),
            mode: None,
        }
    }
}
