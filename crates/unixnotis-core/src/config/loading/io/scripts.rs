//! Provisioning for built-in helper scripts

use std::path::Path;

use crate::filesystem::{make_file_executable, write_file_atomic};
use crate::{Config, DEFAULT_SCRIPTS};

use super::ConfigError;

impl Config {
    /// Ensure helper scripts used by the shipped default config exist
    ///
    /// # Errors
    ///
    /// Returns an error when a missing script cannot be written or made executable
    pub fn ensure_default_scripts_in(config_dir: &Path) -> Result<(), ConfigError> {
        for script in DEFAULT_SCRIPTS {
            let path = config_dir.join(script.relative_path);
            // Existing files are preserved so user-edited helpers are not overwritten
            if !path.exists() {
                write_default_script(&path, script.contents)?;
            }
            // Relative commands run the helper directly, so execute bits must be present
            set_executable(&path)?;
        }
        Ok(())
    }

    /// Overwrite helper scripts with the built-in defaults
    ///
    /// # Errors
    ///
    /// Returns an error when any script cannot be replaced safely
    pub fn write_default_scripts_in(config_dir: &Path) -> Result<(), ConfigError> {
        for script in DEFAULT_SCRIPTS {
            write_default_script(&config_dir.join(script.relative_path), script.contents)?;
        }
        Ok(())
    }
}

fn write_default_script(path: &Path, contents: &str) -> Result<(), ConfigError> {
    // Script reset uses the same atomic path as startup provisioning
    // This keeps installer resets from leaving half-written helpers behind
    write_file_atomic(path, contents.as_bytes(), 0o755)
        .map_err(|err| ConfigError::ReadFailed(err.to_string()))?;
    set_executable(path)
}

#[cfg(unix)]
fn set_executable(path: &Path) -> Result<(), ConfigError> {
    make_file_executable(path).map_err(|err| ConfigError::ReadFailed(err.to_string()))
}

#[cfg(not(unix))]
fn set_executable(_path: &Path) -> Result<(), ConfigError> {
    Ok(())
}
