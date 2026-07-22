//! Shared filesystem operations with stable directory anchors

mod atomic;
mod install;
mod path;

pub use atomic::{
    make_file_executable, set_file_mode, write_file_atomic, write_file_atomic_preserving_mode,
    write_file_if_missing,
};
pub use install::copy_file_atomic;
pub use path::{ContainedPath, LexicalPathError, LexicallyNormalizedPath};
