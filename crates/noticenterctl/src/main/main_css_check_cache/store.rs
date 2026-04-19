use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use super::model::{CachedParseDiagnostic, CssFileIdentity, CssParseWorkItem};

const CSS_PARSE_CACHE_VERSION: u32 = 2;
const CSS_PARSE_CACHE_FILE: &str = "css-check-parse-cache-v2.json";

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct CssParseCacheFile {
    // Versioned on-disk state makes incompatible cache changes cheap to drop
    version: u32,
    entries: BTreeMap<String, CssParseCacheEntry>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct CssParseCacheEntry {
    identity: CssFileIdentity,
    // Hashing cached hits closes the coarse-mtime false-hit edge
    content_hash: String,
    diagnostics: Vec<CachedParseDiagnostic>,
}

pub(in super::super) struct CssParseCacheState {
    // The resolved cache file path stays with the state until save time
    path: PathBuf,
    file: CssParseCacheFile,
    // Dirty state avoids rewriting the cache when nothing changed
    dirty: bool,
}

impl CssParseCacheState {
    pub(in super::super) fn load(path: PathBuf) -> Self {
        // Broken cache files should never block validation
        let file = fs::read_to_string(&path)
            .ok()
            .and_then(|contents| serde_json::from_str::<CssParseCacheFile>(&contents).ok())
            .filter(|cache| cache.version == CSS_PARSE_CACHE_VERSION)
            .unwrap_or_else(|| CssParseCacheFile {
                version: CSS_PARSE_CACHE_VERSION,
                entries: BTreeMap::new(),
            });

        Self {
            path,
            file,
            dirty: false,
        }
    }

    pub(in super::super) fn lookup(
        &self,
        work_item: &CssParseWorkItem,
    ) -> Result<Option<&Vec<CachedParseDiagnostic>>> {
        // Canonical keys collapse aliases back to one real file entry
        let key = cache_key_for_path(&work_item.canonical_path);
        let Some(entry) = self.file.entries.get(&key) else {
            return Ok(None);
        };
        if entry.identity != work_item.identity {
            return Ok(None);
        }

        // A would-be hit still proves the current bytes before reuse
        let current_hash = hash_css_file_bytes(&work_item.load_path)?;
        if current_hash == entry.content_hash {
            return Ok(Some(&entry.diagnostics));
        }

        Ok(None)
    }

    pub(in super::super) fn store(
        &mut self,
        work_item: &CssParseWorkItem,
        diagnostics: Vec<CachedParseDiagnostic>,
    ) -> Result<()> {
        // The same canonical key is reused for fresh writes
        let key = cache_key_for_path(&work_item.canonical_path);
        let entry = CssParseCacheEntry {
            identity: work_item.identity.clone(),
            content_hash: hash_css_file_bytes(&work_item.load_path)?,
            diagnostics,
        };
        if self.file.entries.get(&key) == Some(&entry) {
            return Ok(());
        }
        self.file.entries.insert(key, entry);
        self.dirty = true;
        Ok(())
    }

    pub(in super::super) fn save(self) {
        if !self.dirty {
            return;
        }

        let Some(parent) = self.path.parent() else {
            return;
        };

        if fs::create_dir_all(parent).is_err() {
            return;
        }

        let Ok(contents) = serde_json::to_vec_pretty(&self.file) else {
            return;
        };

        // Write-then-rename keeps partial cache files out of later runs
        let temp_path = parent.join(format!(
            ".{}.tmp-{}",
            CSS_PARSE_CACHE_FILE,
            std::process::id()
        ));
        if fs::write(&temp_path, contents).is_err() {
            return;
        }
        if fs::rename(&temp_path, &self.path).is_err() {
            let _ = fs::remove_file(&temp_path);
        }
    }
}

pub(in super::super) fn default_css_parse_cache_path() -> Option<PathBuf> {
    // Cache storage should follow the usual XDG rules first
    if let Ok(cache_home) = env::var("XDG_CACHE_HOME") {
        let trimmed = cache_home.trim();
        if !trimmed.is_empty() {
            return Some(
                PathBuf::from(trimmed)
                    .join("unixnotis")
                    .join(CSS_PARSE_CACHE_FILE),
            );
        }
    }

    let home = env::var("HOME").ok()?;
    Some(
        PathBuf::from(home)
            .join(".cache")
            .join("unixnotis")
            .join(CSS_PARSE_CACHE_FILE),
    )
}

fn cache_key_for_path(path: &Path) -> String {
    // Canonicalized paths are stored as plain strings for stable json keys
    path.to_string_lossy().into_owned()
}

fn hash_css_file_bytes(path: &Path) -> Result<String> {
    // Hash the exact bytes GTK would read so cached hits stay honest
    let bytes = fs::read(path).with_context(|| format!("read css file {}", path.display()))?;
    Ok(blake3::hash(&bytes).to_hex().to_string())
}
