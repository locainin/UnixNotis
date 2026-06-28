//! User-facing path formatting helpers

use std::path::{Path, PathBuf};

use super::dirs::home_dir;

pub fn format_with_home(path: &Path) -> String {
    if let Ok(home) = home_dir() {
        if let Ok(stripped) = path.strip_prefix(&home) {
            // Logs should avoid leaking the real home directory when $HOME is enough
            let mut rendered = PathBuf::from("$HOME");
            rendered.push(stripped);
            return rendered.display().to_string();
        }
    }
    // Non-home paths are left intact because they may point at /run, /tmp, or custom roots
    path.display().to_string()
}
