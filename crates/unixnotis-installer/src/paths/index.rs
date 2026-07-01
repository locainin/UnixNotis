//! Filesystem layout helpers for UnixNotis installation paths

mod choice;
mod dirs;
mod discovery;
mod format;

pub use choice::ServiceManagerChoice;
pub use dirs::home_dir;
pub use discovery::InstallPaths;
pub use format::format_with_home;

#[cfg(test)]
use discovery::is_unixnotis_repo;

#[cfg(test)]
#[path = "tests/index.rs"]
mod tests;
