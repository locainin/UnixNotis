//! Preset filesystem module root
//!
//! Keeps filesystem validation and secure write helpers grouped under one tree
//! so callers can import one module name instead of a flat list of files

mod checks;
mod secure;

pub(super) use self::checks::{
    ensure_dir_fd_matches_live_path, ensure_no_symlink_ancestors, ensure_safe_target_path,
};
pub(super) use self::secure::{
    create_backup_dir_secure, open_secure_dir_all, publish_relative_file_atomic_secure,
    read_relative_file_secure, read_relative_file_secure_bounded,
    remove_empty_relative_dirs_secure, remove_relative_dir_secure, remove_relative_file_secure,
    write_relative_file_atomic_secure,
};

#[cfg(test)]
mod tests;
