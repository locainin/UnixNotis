//! Cache-aware GTK parse stage for css-check

use anyhow::{Context, Result};
use gtk::prelude::*;
use gtk::CssProvider;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use super::main_css_check_files::format_display_path;
use super::main_css_check_policy::parsing_error_hint;
use super::main_css_check_report::{CssCheckCategory, CssCheckDiagnostic};
use super::source_line_text;

const CSS_PARSE_CACHE_VERSION: u32 = 2;
const CSS_PARSE_CACHE_FILE: &str = "css-check-parse-cache-v2.json";

pub(super) struct CssParseReport {
    pub(super) diagnostics: Vec<CssCheckDiagnostic>,
    pub(super) error_count: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct CssParseWorkItem {
    // The configured path stays as the visible path in fresh and cached reports
    load_path: PathBuf,
    // Canonical paths keep aliases and symlinks on one cache key
    canonical_path: PathBuf,
    // Stable identity keeps stale success and stale failure entries out
    identity: CssFileIdentity,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct CssFileIdentity {
    size: u64,
    modified_nanos: u128,
    #[cfg(unix)]
    device: u64,
    #[cfg(unix)]
    inode: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
enum CachedDiagnosticSource {
    // Top-level findings should always render against the current logical input path
    TopLevel,
    // Imported files need their own stable path in the report
    Path(PathBuf),
    // GTK can occasionally report inline data instead of a file path
    Data,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct CachedParseDiagnostic {
    source: CachedDiagnosticSource,
    line: Option<usize>,
    column: Option<usize>,
    message: String,
    hint: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct CssParseCacheFile {
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

struct CssParseCacheState {
    path: PathBuf,
    file: CssParseCacheFile,
    dirty: bool,
}

impl CssParseCacheState {
    fn load(path: PathBuf) -> Self {
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

    fn lookup(&self, work_item: &CssParseWorkItem) -> Result<Option<&CssParseCacheEntry>> {
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
            return Ok(Some(entry));
        }

        Ok(None)
    }

    fn store(
        &mut self,
        work_item: &CssParseWorkItem,
        diagnostics: Vec<CachedParseDiagnostic>,
    ) -> Result<()> {
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

    fn save(self) {
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

pub(super) fn validate_css_parse_files(
    files: &[PathBuf],
    config_dir: &Path,
    display_root: &str,
) -> Result<CssParseReport> {
    let work_items = build_parse_work_items(files)?;
    let cache_path = default_css_parse_cache_path();
    run_cached_parse_session(
        &work_items,
        config_dir,
        display_root,
        cache_path.as_deref(),
        parse_css_file_with_gtk,
    )
}

fn run_cached_parse_session<F>(
    work_items: &[CssParseWorkItem],
    config_dir: &Path,
    display_root: &str,
    cache_path: Option<&Path>,
    mut parse_file: F,
) -> Result<CssParseReport>
where
    F: FnMut(&CssParseWorkItem) -> Result<Vec<CachedParseDiagnostic>>,
{
    let mut cache = cache_path.map(|path| CssParseCacheState::load(path.to_path_buf()));
    let mut diagnostics = Vec::new();
    let mut error_count = 0usize;

    for work_item in work_items {
        if let Some(entry) = cache
            .as_ref()
            .map(|cache| cache.lookup(work_item))
            .transpose()?
            .flatten()
        {
            let cached_diagnostics =
                render_cached_diagnostics(&entry.diagnostics, work_item, config_dir, display_root);
            error_count += cached_diagnostics.len();
            diagnostics.extend(cached_diagnostics);
            continue;
        }

        let fresh_diagnostics = parse_file(work_item)?;
        error_count += fresh_diagnostics.len();
        diagnostics.extend(render_cached_diagnostics(
            &fresh_diagnostics,
            work_item,
            config_dir,
            display_root,
        ));
        if let Some(cache) = cache.as_mut() {
            cache.store(work_item, fresh_diagnostics)?;
        }
    }

    if let Some(cache) = cache {
        cache.save();
    }

    Ok(CssParseReport {
        diagnostics,
        error_count,
    })
}

fn build_parse_work_items(files: &[PathBuf]) -> Result<Vec<CssParseWorkItem>> {
    let mut work_items = Vec::with_capacity(files.len());
    for path in files {
        // Metadata should come from the real target, not the symlink shell
        let metadata =
            fs::metadata(path).with_context(|| format!("read css metadata {}", path.display()))?;
        let canonical_path = fs::canonicalize(path)
            .with_context(|| format!("resolve css file {}", path.display()))?;
        work_items.push(CssParseWorkItem {
            load_path: path.clone(),
            canonical_path,
            identity: CssFileIdentity::from_metadata(&metadata)?,
        });
    }
    Ok(work_items)
}

fn parse_css_file_with_gtk(work_item: &CssParseWorkItem) -> Result<Vec<CachedParseDiagnostic>> {
    let provider = CssProvider::new();
    let current_file = work_item.canonical_path.clone();
    let findings = std::rc::Rc::new(std::cell::RefCell::new(Vec::<CachedParseDiagnostic>::new()));
    let findings_for_signal = findings.clone();

    provider.connect_parsing_error(move |_provider, section, error| {
        let location = section.start_location();
        let line = location.lines() + 1;
        let source_path = section.file().and_then(|file| file.path());

        // Line hints stay tied to the exact file GTK blamed
        let hint = source_line_text(source_path.as_deref(), line)
            .and_then(|line_text| parsing_error_hint(&line_text));

        let source = classify_cached_source_path(source_path.as_deref(), &current_file);
        findings_for_signal
            .borrow_mut()
            .push(CachedParseDiagnostic {
                source,
                line: Some(line),
                column: Some(location.line_chars() + 1),
                message: error.message().to_string(),
                hint,
            });
    });

    // Gtk clears prior provider state on every load_from_path call
    provider.load_from_path(&work_item.load_path);
    let diagnostics = findings.borrow().clone();
    Ok(diagnostics)
}

fn classify_cached_source_path(
    source_path: Option<&Path>,
    current_file: &Path,
) -> CachedDiagnosticSource {
    let Some(source_path) = source_path else {
        return CachedDiagnosticSource::Data;
    };

    let normalized_source =
        fs::canonicalize(source_path).unwrap_or_else(|_| source_path.to_path_buf());
    if normalized_source == current_file {
        return CachedDiagnosticSource::TopLevel;
    }

    CachedDiagnosticSource::Path(source_path.to_path_buf())
}

fn render_cached_diagnostics(
    diagnostics: &[CachedParseDiagnostic],
    work_item: &CssParseWorkItem,
    config_dir: &Path,
    display_root: &str,
) -> Vec<CssCheckDiagnostic> {
    let top_level_display = format_display_path(config_dir, display_root, &work_item.load_path);
    diagnostics
        .iter()
        .map(|diagnostic| {
            let display_path = match &diagnostic.source {
                CachedDiagnosticSource::TopLevel => top_level_display.clone(),
                CachedDiagnosticSource::Path(path) => {
                    format_display_path(config_dir, display_root, path)
                }
                CachedDiagnosticSource::Data => "<data>".to_string(),
            };

            CssCheckDiagnostic::error(
                CssCheckCategory::Parse,
                display_path,
                diagnostic.line,
                diagnostic.column,
                diagnostic.message.clone(),
                diagnostic.hint.clone(),
            )
        })
        .collect()
}

fn default_css_parse_cache_path() -> Option<PathBuf> {
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
    path.to_string_lossy().into_owned()
}

fn hash_css_file_bytes(path: &Path) -> Result<String> {
    // Hash the exact bytes GTK would read so cached hits stay honest
    let bytes = fs::read(path).with_context(|| format!("read css file {}", path.display()))?;
    Ok(blake3::hash(&bytes).to_hex().to_string())
}

impl CssFileIdentity {
    fn from_metadata(metadata: &fs::Metadata) -> Result<Self> {
        let modified = metadata
            .modified()
            .context("read css file modification time")?;
        let modified_nanos = modified
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();

        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;

            return Ok(Self {
                size: metadata.len(),
                modified_nanos,
                device: metadata.dev(),
                inode: metadata.ino(),
            });
        }

        #[cfg(not(unix))]
        {
            Ok(Self {
                size: metadata.len(),
                modified_nanos,
            })
        }
    }
}

#[cfg(test)]
fn validate_css_parse_files_with(
    files: &[PathBuf],
    config_dir: &Path,
    display_root: &str,
    cache_path: &Path,
    parse_file: impl FnMut(&CssParseWorkItem) -> Result<Vec<CachedParseDiagnostic>>,
) -> Result<CssParseReport> {
    let work_items = build_parse_work_items(files)?;
    run_cached_parse_session(
        &work_items,
        config_dir,
        display_root,
        Some(cache_path),
        parse_file,
    )
}

#[cfg(test)]
fn parse_diagnostic_for_test(message: impl Into<String>) -> Vec<CachedParseDiagnostic> {
    vec![CachedParseDiagnostic {
        source: CachedDiagnosticSource::TopLevel,
        line: Some(1),
        column: Some(1),
        message: message.into(),
        hint: None,
    }]
}

#[cfg(test)]
#[path = "main_css_check_cache_tests.rs"]
mod tests;
