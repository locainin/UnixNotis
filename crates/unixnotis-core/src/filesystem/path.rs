//! Lexical path normalization and root containment without filesystem access

use std::path::{Component, Path, PathBuf};

use thiserror::Error;

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct LexicallyNormalizedPath(PathBuf);

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct ContainedPath {
    root: LexicallyNormalizedPath,
    relative: PathBuf,
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum LexicalPathError {
    #[error("path parent traversal escapes its lexical root")]
    ParentEscape,
    #[error("contained path must be relative")]
    ExpectedRelative,
    #[error("path is outside the supplied root")]
    OutsideRoot,
}

impl LexicallyNormalizedPath {
    /// Normalize `.` and `..` components without resolving symlinks
    ///
    /// # Errors
    ///
    /// Returns an error when parent traversal would escape the lexical path root
    pub fn new(path: impl AsRef<Path>) -> Result<Self, LexicalPathError> {
        let mut normalized = PathBuf::new();
        for component in path.as_ref().components() {
            match component {
                Component::CurDir => {}
                Component::ParentDir => match normalized.components().next_back() {
                    Some(Component::Normal(_)) => {
                        let removed = normalized.pop();
                        debug_assert!(removed, "normal path component must be removable");
                    }
                    _ => return Err(LexicalPathError::ParentEscape),
                },
                Component::Normal(part) => normalized.push(part),
                Component::RootDir | Component::Prefix(_) => {
                    normalized.push(component.as_os_str());
                }
            }
        }
        Ok(Self(normalized))
    }

    #[must_use]
    pub fn as_path(&self) -> &Path {
        self.0.as_path()
    }

    #[must_use]
    pub fn into_path_buf(self) -> PathBuf {
        self.0
    }
}

impl AsRef<Path> for LexicallyNormalizedPath {
    fn as_ref(&self) -> &Path {
        self.as_path()
    }
}

impl ContainedPath {
    /// Resolve an absolute or relative candidate beneath one lexical root
    ///
    /// # Errors
    ///
    /// Returns an error when normalization fails or the result leaves `root`
    pub fn resolve(
        root: impl AsRef<Path>,
        candidate: impl AsRef<Path>,
    ) -> Result<Self, LexicalPathError> {
        let root = LexicallyNormalizedPath::new(root)?;
        let candidate = candidate.as_ref();
        let joined = if candidate.is_absolute() {
            candidate.to_path_buf()
        } else {
            root.as_path().join(candidate)
        };
        let normalized = LexicallyNormalizedPath::new(joined)?;
        let relative = normalized
            .as_path()
            .strip_prefix(root.as_path())
            .map_err(|_outside_root| LexicalPathError::OutsideRoot)?
            .to_path_buf();
        Ok(Self { root, relative })
    }

    /// Resolve a candidate that must be relative beneath one lexical root
    ///
    /// # Errors
    ///
    /// Returns an error for absolute candidates, traversal, or containment failure
    pub fn resolve_relative(
        root: impl AsRef<Path>,
        relative: impl AsRef<Path>,
    ) -> Result<Self, LexicalPathError> {
        if relative.as_ref().is_absolute() {
            return Err(LexicalPathError::ExpectedRelative);
        }
        Self::resolve(root, relative)
    }

    #[must_use]
    pub fn root(&self) -> &Path {
        self.root.as_path()
    }

    #[must_use]
    pub fn relative(&self) -> &Path {
        self.relative.as_path()
    }

    #[must_use]
    pub fn absolute(&self) -> PathBuf {
        self.root.as_path().join(&self.relative)
    }
}

#[cfg(test)]
#[path = "tests/path.rs"]
mod tests;
