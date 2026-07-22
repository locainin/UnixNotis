//! Shared filesystem operations with stable directory anchors

mod atomic;
mod directory;
mod install;
mod path;
mod remove;
mod symlink;

pub use atomic::{
    make_file_executable, set_file_mode, write_file_atomic, write_file_atomic_preserving_mode,
    write_file_if_missing,
};
pub use directory::{create_directory_all, remove_directory_tree, remove_empty_directory};
pub use install::copy_file_atomic;
pub use path::{ContainedPath, LexicalPathError, LexicallyNormalizedPath};
pub use remove::{
    remove_regular_file, remove_symlink, remove_symlink_if_target, RemoveSymlinkOutcome,
};
pub use symlink::{
    create_symlink_if_missing, read_symlink, replace_symlink_atomic, CreateSymlinkOutcome,
};
