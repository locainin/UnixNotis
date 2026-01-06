//! Shared helper utilities used across UnixNotis components.

use std::collections::HashMap;
use std::env;
use std::path::Path;
use std::sync::{Mutex, OnceLock};

static PROGRAM_CACHE: OnceLock<Mutex<HashMap<String, bool>>> = OnceLock::new();

/// Check whether a program exists in $PATH, caching results to avoid repeated scans.
pub fn program_in_path(program: &str) -> bool {
    if program.contains(std::path::MAIN_SEPARATOR) {
        return Path::new(program).is_file();
    }
    let cache = PROGRAM_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Ok(cache) = cache.lock() {
        if let Some(result) = cache.get(program) {
            return *result;
        }
    }

    let found = match env::var("PATH") {
        Ok(paths) => env::split_paths(&paths)
            .any(|dir| dir.join(program).is_file()),
        Err(_) => false,
    };

    if let Ok(mut cache) = cache.lock() {
        cache.insert(program.to_string(), found);
    }

    found
}
