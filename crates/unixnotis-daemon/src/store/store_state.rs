//! DND persistence helpers.
//!
//! Encapsulates on-disk state to keep filesystem I/O isolated from the store core.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use chrono::Utc;
use serde::{Deserialize, Serialize};

pub(super) const DND_STATE_VERSION: u32 = 1;
pub(super) const DND_STATE_FILE: &str = "state.json";

#[derive(Debug, Serialize, Deserialize)]
pub(super) struct PersistedDndState {
    pub(super) version: u32,
    pub(super) dnd_enabled: bool,
    pub(super) updated_at: Option<String>,
}

#[derive(Debug, Clone)]
pub(super) struct DndStateStore {
    path: PathBuf,
}

impl DndStateStore {
    pub(super) fn new() -> Option<Self> {
        let state_dir = resolve_state_dir()?;
        Some(Self::from_state_dir(state_dir))
    }

    pub(super) fn from_state_dir(state_dir: PathBuf) -> Self {
        let path = state_dir.join("unixnotis").join(DND_STATE_FILE);
        Self { path }
    }

    pub(super) fn load(&self) -> io::Result<Option<PersistedDndState>> {
        let contents = match fs::read_to_string(&self.path) {
            Ok(contents) => contents,
            Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(err) => return Err(err),
        };
        let parsed = serde_json::from_str(&contents)
            .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;
        Ok(Some(parsed))
    }

    pub(super) fn persist(&self, enabled: bool) -> io::Result<()> {
        let payload = PersistedDndState {
            version: DND_STATE_VERSION,
            dnd_enabled: enabled,
            updated_at: Some(Utc::now().to_rfc3339()),
        };
        let body = serde_json::to_vec(&payload)?;
        let parent = match self.path.parent() {
            Some(parent) => parent,
            None => return Ok(()),
        };
        fs::create_dir_all(parent)?;
        let temp_path = self.temp_path(parent);
        let mut file = fs::File::create(&temp_path)?;
        // Write and flush the file before renaming to avoid partially written state files.
        io::Write::write_all(&mut file, &body)?;
        // Best-effort fsync keeps the temp file durable across sudden power loss.
        let _ = file.sync_all();
        fs::rename(&temp_path, &self.path)?;
        // Sync the directory entry to keep the rename durable on supported filesystems.
        let _ = sync_parent_dir(parent);
        Ok(())
    }

    fn temp_path(&self, parent: &Path) -> PathBuf {
        let pid = std::process::id();
        let nanos = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0);
        let name = format!(".{DND_STATE_FILE}.tmp.{pid}.{nanos}");
        parent.join(name)
    }
}

fn resolve_state_dir() -> Option<PathBuf> {
    if let Ok(dir) = std::env::var("XDG_STATE_HOME") {
        if !dir.trim().is_empty() {
            return Some(PathBuf::from(dir));
        }
    }
    let home = std::env::var("HOME").ok()?;
    if home.trim().is_empty() {
        return None;
    }
    Some(PathBuf::from(home).join(".local").join("state"))
}

fn sync_parent_dir(parent: &Path) -> io::Result<()> {
    let dir = fs::File::open(parent)?;
    dir.sync_all()
}
