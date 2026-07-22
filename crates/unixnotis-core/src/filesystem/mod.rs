//! Shared filesystem operations with stable directory anchors

mod atomic;
mod install;
mod path;
mod remove;

pub use atomic::{
    make_file_executable, set_file_mode, write_file_atomic, write_file_atomic_preserving_mode,
    write_file_if_missing,
};
pub use install::copy_file_atomic;
pub use path::{ContainedPath, LexicalPathError, LexicallyNormalizedPath};
pub use remove::{
    read_symlink, remove_regular_file, remove_symlink, remove_symlink_if_target,
    RemoveSymlinkOutcome,
};
