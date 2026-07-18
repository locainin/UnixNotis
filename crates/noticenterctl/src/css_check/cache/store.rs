use anyhow::Result;
use rustix::fs::{open, Mode, OFlags};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::io::Read as _;
use std::path::{Path, PathBuf};
use unixnotis_core::filesystem::write_file_atomic;

use super::model::{CachedParseDiagnostic, CssDependencyState, CssFileIdentity, CssParseWorkItem};

const CSS_PARSE_CACHE_VERSION: u32 = 2;
const CSS_PARSE_CACHE_FILE: &str = "css-check-parse-cache-v2.json";
pub(in super::super) const CSS_PARSE_CACHE_MAX_BYTES: usize = 8 * 1024 * 1024;
pub(in super::super) const CSS_PARSE_CACHE_MAX_ENTRIES: usize = 256;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct CssParseCacheFile {
    // Versioned on-disk state makes incompatible cache changes cheap to drop
    version: u32,
    #[serde(default)]
    access_counter: u64,
    entries: BTreeMap<String, CssParseCacheEntry>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct CssParseCacheEntry {
    identity: CssFileIdentity,
    content_hash: String,
    // Imported files have to match too or stale findings leak through later runs
    dependencies: Vec<CssDependencyState>,
    diagnostics: Vec<CachedParseDiagnostic>,
    // Monotonic access order makes bounded eviction deterministic across runs
    #[serde(default)]
    last_used: u64,
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
        let mut file = read_cache_file_bounded(&path)
            .and_then(|contents| serde_json::from_slice::<CssParseCacheFile>(&contents).ok())
            .filter(|cache| cache.version == CSS_PARSE_CACHE_VERSION)
            .unwrap_or_else(empty_cache_file);
        let mut dirty = normalize_access_counter(&mut file);
        dirty |= prune_lru_entries(&mut file.entries, CSS_PARSE_CACHE_MAX_ENTRIES);

        Self { path, file, dirty }
    }

    pub(in super::super) fn lookup(
        &mut self,
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

        // A successful hit becomes the newest entry before any later insertion can evict it
        let last_used = self.next_access();
        let entry = self
            .file
            .entries
            .get_mut(&key)
            .expect("validated cache entry must still exist");
        entry.last_used = last_used;
        self.dirty = true;
        Ok(Some(&entry.diagnostics))
    }

    pub(in super::super) fn store(
        &mut self,
        work_item: &CssParseWorkItem,
        diagnostics: Vec<CachedParseDiagnostic>,
    ) -> Result<()> {
        // The same canonical key is reused for fresh writes
        let key = cache_key_for_path(&work_item.canonical_path);
        if self.file.entries.get(&key).is_some_and(|entry| {
            entry.identity == work_item.identity
                && entry.content_hash == work_item.content_hash
                && entry.dependencies == work_item.dependencies
                && entry.diagnostics == diagnostics
        }) {
            let last_used = self.next_access();
            if let Some(entry) = self.file.entries.get_mut(&key) {
                entry.last_used = last_used;
            }
            self.dirty = true;
            return Ok(());
        }

        let entry = CssParseCacheEntry {
            identity: work_item.identity.clone(),
            content_hash: work_item.content_hash.clone(),
            dependencies: work_item.dependencies.clone(),
            diagnostics,
            last_used: self.next_access(),
        };
        self.file.entries.insert(key, entry);
        prune_lru_entries(&mut self.file.entries, CSS_PARSE_CACHE_MAX_ENTRIES);
        self.dirty = true;
        Ok(())
    }

    pub(in super::super) fn save(mut self) {
        if !self.dirty {
            return;
        }

        let Some(contents) = serialize_cache_bounded(&mut self.file) else {
            return;
        };
        // Cache persistence is optional, but successful writes still use the hardened path
        let _ = write_file_atomic(&self.path, &contents, 0o600);
    }

    fn next_access(&mut self) -> u64 {
        if self.file.access_counter == u64::MAX {
            // Renumbering preserves order and prevents a saturated counter from flattening LRU
            let mut keys = self
                .file
                .entries
                .iter()
                .map(|(key, entry)| (key.clone(), entry.last_used))
                .collect::<Vec<_>>();
            keys.sort_by(|left, right| left.1.cmp(&right.1).then_with(|| left.0.cmp(&right.0)));
            for (index, (key, _)) in keys.into_iter().enumerate() {
                if let Some(entry) = self.file.entries.get_mut(&key) {
                    entry.last_used = u64::try_from(index.saturating_add(1)).unwrap_or(u64::MAX);
                }
            }
            self.file.access_counter = u64::try_from(self.file.entries.len()).unwrap_or(u64::MAX);
        }
        self.file.access_counter = self.file.access_counter.saturating_add(1);
        self.file.access_counter
    }
}

const fn empty_cache_file() -> CssParseCacheFile {
    CssParseCacheFile {
        version: CSS_PARSE_CACHE_VERSION,
        access_counter: 0,
        entries: BTreeMap::new(),
    }
}

fn read_cache_file_bounded(path: &Path) -> Option<Vec<u8>> {
    // Nonblocking no-follow open rejects FIFOs and links before optional cache data is read
    let fd = open(
        path,
        OFlags::RDONLY
            .union(OFlags::CLOEXEC)
            .union(OFlags::NONBLOCK)
            .union(OFlags::NOFOLLOW),
        Mode::empty(),
    )
    .ok()?;
    let mut file = fs::File::from(fd);
    let metadata = file.metadata().ok()?;
    if !metadata.is_file()
        || metadata.len() > u64::try_from(CSS_PARSE_CACHE_MAX_BYTES).unwrap_or(u64::MAX)
    {
        return None;
    }

    let read_limit = u64::try_from(CSS_PARSE_CACHE_MAX_BYTES)
        .unwrap_or(u64::MAX)
        .saturating_add(1);
    let mut contents = Vec::with_capacity(
        usize::try_from(metadata.len())
            .unwrap_or(CSS_PARSE_CACHE_MAX_BYTES)
            .min(CSS_PARSE_CACHE_MAX_BYTES),
    );
    file.by_ref()
        .take(read_limit)
        .read_to_end(&mut contents)
        .ok()?;
    (contents.len() <= CSS_PARSE_CACHE_MAX_BYTES).then_some(contents)
}

fn normalize_access_counter(file: &mut CssParseCacheFile) -> bool {
    let newest_entry = file
        .entries
        .values()
        .map(|entry| entry.last_used)
        .max()
        .unwrap_or(0);
    if file.access_counter >= newest_entry {
        return false;
    }
    file.access_counter = newest_entry;
    true
}

fn prune_lru_entries(entries: &mut BTreeMap<String, CssParseCacheEntry>, limit: usize) -> bool {
    let mut pruned = false;
    while entries.len() > limit {
        let Some(oldest_key) = entries
            .iter()
            .min_by(|left, right| {
                left.1
                    .last_used
                    .cmp(&right.1.last_used)
                    .then_with(|| left.0.cmp(right.0))
            })
            .map(|(key, _)| key.clone())
        else {
            break;
        };
        entries.remove(&oldest_key);
        pruned = true;
    }
    pruned
}

fn serialize_cache_bounded(file: &mut CssParseCacheFile) -> Option<Vec<u8>> {
    loop {
        let contents = serde_json::to_vec_pretty(file).ok()?;
        if contents.len() <= CSS_PARSE_CACHE_MAX_BYTES {
            return Some(contents);
        }
        // Halving bounds serialization retries even when many diagnostics are individually large
        let next_limit = file.entries.len() / 2;
        if !prune_lru_entries(&mut file.entries, next_limit) {
            return None;
        }
    }
}

pub(in super::super) fn default_css_parse_cache_path() -> Option<PathBuf> {
    // Cache storage should follow the usual XDG rules first
    if let Ok(cache_home) = env::var("XDG_CACHE_HOME") {
        let trimmed = cache_home.trim();
        let cache_home = PathBuf::from(trimmed);
        if cache_home.is_absolute() {
            return Some(cache_home.join("unixnotis").join(CSS_PARSE_CACHE_FILE));
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
