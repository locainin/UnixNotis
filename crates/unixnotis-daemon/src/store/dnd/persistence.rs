//! DND persistence helpers
//!
//! Encapsulates on-disk state to keep filesystem I/O isolated from the store core

use std::fs;
use std::io;
use std::path::PathBuf;

use chrono::Utc;
use serde::{Deserialize, Serialize};
use unixnotis_core::filesystem::write_file_atomic;
use unixnotis_core::util;

pub(in crate::store) const DND_STATE_VERSION: u32 = 1;
pub(in crate::store) const DND_STATE_FILE: &str = "state.json";

#[derive(Debug, Serialize, Deserialize)]
pub(in crate::store) struct PersistedDndState {
    pub(in crate::store) version: u32,
    pub(in crate::store) dnd_enabled: bool,
    #[serde(default)]
    pub(in crate::store) expires_at: Option<i64>,
    pub(in crate::store) updated_at: Option<String>,
}

#[derive(Debug, Clone)]
pub struct DndStateStore {
    path: PathBuf,
}

impl DndStateStore {
    pub(in crate::store) fn new() -> Option<Self> {
        let state_dir = util::resolve_state_dir()?;
        Some(Self::from_state_dir(state_dir))
    }

    pub(in crate::store) fn from_state_dir(state_dir: PathBuf) -> Self {
        let path = state_dir.join("unixnotis").join(DND_STATE_FILE);
        Self { path }
    }

    pub(in crate::store) fn load(&self) -> io::Result<Option<PersistedDndState>> {
        let contents = match fs::read_to_string(&self.path) {
            Ok(contents) => contents,
            Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(err) => return Err(err),
        };
        let parsed = serde_json::from_str(&contents)
            .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;
        Ok(Some(parsed))
    }

    pub(crate) fn persist(&self, enabled: bool, expires_at: Option<i64>) -> io::Result<()> {
        let payload = PersistedDndState {
            version: DND_STATE_VERSION,
            dnd_enabled: enabled,
            // Disabled state never keeps a stale deadline on disk
            expires_at: enabled.then_some(expires_at).flatten(),
            updated_at: Some(Utc::now().to_rfc3339()),
        };
        let body = serde_json::to_vec(&payload)?;
        // State is private to the current user and durable across sudden restarts
        write_file_atomic(&self.path, &body, 0o600)
    }
}
