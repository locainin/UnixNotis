use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use unixnotis_core::filesystem::write_file_atomic;

use super::model::{CachedParseDiagnostic, CssDependencyState, CssFileIdentity, CssParseWorkItem};

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
    content_hash: String,
    // Imported files have to match too or stale findings leak through later runs
    dependencies: Vec<CssDependencyState>,
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
        if entry.content_hash != work_item.content_hash {
            return Ok(None);
        }
        if entry.dependencies != work_item.dependencies {
            return Ok(None);
        }

        Ok(Some(&entry.diagnostics))
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
            content_hash: work_item.content_hash.clone(),
            dependencies: work_item.dependencies.clone(),
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

        let Ok(contents) = serde_json::to_vec_pretty(&self.file) else {
            return;
        };
        // Cache persistence is optional, but successful writes still use the hardened path
        let _ = write_file_atomic(&self.path, &contents, 0o600);
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
