//! Shared filesystem operations with stable directory anchors

mod atomic;
mod directory;
mod exact;
mod install;
mod path;
mod read;
mod regular;
mod remove;
mod rename;
mod symlink;

pub use atomic::{write_file_atomic, write_file_atomic_preserving_mode, write_file_if_missing};
pub use directory::{
    create_directory_all, ensure_marked_directory, remove_directory_tree, remove_empty_directory,
    remove_marked_directory_tree, CreateDirectoryOutcome,
};
pub use exact::{
    ensure_exact_file, ensure_exact_file_pair, EnsureExactFileOutcome, EnsureExactFilePairOutcome,
};
pub use install::copy_file_atomic;
pub use path::{ContainedPath, LexicalPathError, LexicallyNormalizedPath};
pub use read::read_regular_file_bounded;
pub use regular::{
    make_file_executable, open_regular_file, regular_file_contents_equal, set_file_mode,
};
pub use remove::{
    remove_regular_file, remove_regular_file_pair_if_contents, remove_symlink,
    remove_symlink_if_target, RemoveExactFileOutcome, RemoveSymlinkOutcome,
};
pub use rename::{rename_regular_file_no_replace, RenameRegularFileOutcome};
pub use symlink::{
    create_symlink_if_missing, read_symlink, replace_symlink_atomic, CreateSymlinkOutcome,
};
