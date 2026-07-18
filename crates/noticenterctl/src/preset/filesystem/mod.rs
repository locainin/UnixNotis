//! Preset filesystem validation and descriptor-relative operations

mod checks;
mod secure;

pub(super) use self::checks::{
    ensure_dir_fd_matches_live_path, ensure_no_symlink_ancestors, ensure_safe_target_path,
};
pub(super) use self::secure::{
    create_backup_dir_secure, open_secure_dir_all, publish_relative_file_atomic_secure,
    read_relative_file_secure_bounded, remove_empty_relative_dirs_secure,
    remove_relative_dir_secure, remove_relative_file_secure, try_read_relative_file_secure,
    write_relative_file_atomic_secure, PublishedRelativeFile,
};

#[cfg(test)]
mod tests;
