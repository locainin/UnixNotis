//! Shared filesystem operations with stable directory anchors

mod atomic;
mod descriptor;
mod directory;
mod exact;
mod install;
mod path;
mod quarantine;
mod regular;
mod remove;
mod rename;
mod symlink;
mod tree;

pub use atomic::{write_file_atomic, write_file_atomic_preserving_mode, write_file_if_missing};
pub use descriptor::CreateDirectoryOutcome;
pub use directory::{create_directory_all, ensure_marked_directory, remove_empty_directory};
pub use exact::{
    ensure_exact_file, ensure_exact_file_pair, EnsureExactFileOutcome, EnsureExactFilePairOutcome,
};
pub use install::copy_file_atomic;
pub use path::{ContainedPath, LexicalPathError, LexicallyNormalizedPath};
pub use regular::{
    make_file_executable, open_regular_file, read_regular_file_bounded,
    regular_file_contents_equal, set_file_mode,
};
pub use remove::{
    remove_regular_file, remove_regular_file_pair_if_contents, remove_symlink,
    remove_symlink_if_target, RemoveExactFileOutcome, RemoveSymlinkOutcome,
};
pub use rename::{
    rename_directory_no_replace, rename_regular_file_no_replace, RenameDirectoryOutcome,
    RenameRegularFileOutcome,
};
pub use symlink::{
    create_symlink_if_missing, read_symlink, replace_symlink_atomic, CreateSymlinkOutcome,
};
pub use tree::{remove_directory_tree, remove_marked_directory_tree};
