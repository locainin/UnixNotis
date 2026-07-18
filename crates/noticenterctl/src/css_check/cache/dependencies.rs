use anyhow::{Context, Result};
use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use super::model::{CssDependencyState, CssFileIdentity};
use crate::preset::css_asset_refs::{
    classify_file_url, collect_import_dependency_values, read_css_file_bounded,
    read_css_path_text_bounded, CssImportReference, FileUrlClassification,
};

const MAX_CSS_IMPORT_FILES: usize = 256;
const MAX_CSS_IMPORT_DEPTH: usize = 32;
const MAX_CSS_IMPORT_TOTAL_BYTES: u64 = 67_108_864;

pub(in super::super) fn collect_import_dependency_states(
    css_path: &Path,
) -> Result<Vec<CssDependencyState>> {
    // Sorted dependency state gives cache keys deterministic ordering across runs
    let mut dependencies = BTreeMap::new();
    let mut visited = HashSet::new();
    let mut total_bytes = 0u64;
    // Seed the root so an import cycle back to the first file terminates immediately
    visited.insert(canonical_or_resolved_path(css_path)?);
    collect_import_dependency_states_from(
        css_path,
        0,
        &mut visited,
        &mut dependencies,
        &mut total_bytes,
    )?;
    Ok(dependencies.into_values().collect())
}

fn collect_import_dependency_states_from(
    css_path: &Path,
    depth: usize,
    visited: &mut HashSet<PathBuf>,
    dependencies: &mut BTreeMap<String, CssDependencyState>,
    total_bytes: &mut u64,
) -> Result<()> {
    // Depth is checked before reading so a chain cannot consume one extra file over budget
    if depth > MAX_CSS_IMPORT_DEPTH {
        anyhow::bail!("CSS import depth exceeds {MAX_CSS_IMPORT_DEPTH}");
    }
    // Every stylesheet uses the same regular-file and byte-limit contract
    let contents = read_css_path_text_bounded(css_path)?;

    for import_path in imported_css_paths(&contents, css_path)? {
        // Capture identity and bytes from one descriptor before updating the cache model
        let dependency = CssDependencyState::from_resolved_path(&import_path)?;
        let dependency_key = dependency.path.clone();
        let dependency_sort_key = dependency_key.to_string_lossy().into_owned();
        let recurse = dependency.identity.is_some();

        if !visited.insert(dependency_key) {
            // Repeated imports and cycles share one dependency entry
            continue;
        }
        if dependencies.len() >= MAX_CSS_IMPORT_FILES {
            anyhow::bail!("CSS imports exceed {MAX_CSS_IMPORT_FILES} files");
        }
        if let Some(identity) = dependency.identity.as_ref() {
            // Missing optional imports have no bytes and do not consume the byte budget
            *total_bytes = total_bytes
                .checked_add(identity.size)
                .filter(|total| *total <= MAX_CSS_IMPORT_TOTAL_BYTES)
                .ok_or_else(|| {
                    anyhow::anyhow!("CSS imports exceed {MAX_CSS_IMPORT_TOTAL_BYTES} total bytes")
                })?;
        }

        dependencies
            .entry(dependency_sort_key)
            .or_insert(dependency);

        if recurse {
            // Only existing regular imports can carry another dependency layer
            collect_import_dependency_states_from(
                &import_path,
                depth + 1,
                visited,
                dependencies,
                total_bytes,
            )?;
        }
    }

    Ok(())
}

pub(super) fn imported_css_paths(contents: &str, css_path: &Path) -> Result<Vec<PathBuf>> {
    let mut paths = Vec::new();

    for reference in collect_import_dependency_values(contents)? {
        // Dynamic or escaped imports cannot be resolved safely and are reported elsewhere
        let CssImportReference::Target(target) = reference else {
            continue;
        };
        if let Some(path) = resolve_import_target(css_path, &target) {
            if paths.len() >= MAX_CSS_IMPORT_FILES {
                anyhow::bail!("CSS imports exceed {MAX_CSS_IMPORT_FILES} files");
            }
            paths.push(path);
        }
    }

    Ok(paths)
}

fn resolve_import_target(css_path: &Path, target: &str) -> Option<PathBuf> {
    let trimmed = target.trim();
    if trimmed.is_empty() {
        return None;
    }
    if trimmed.starts_with("//") || trimmed.starts_with("data:") {
        // Remote and embedded imports are runtime concerns rather than local cache dependencies
        return None;
    }
    match classify_file_url(trimmed) {
        FileUrlClassification::Local(path) => return Some(path),
        FileUrlClassification::NonLocalAuthority | FileUrlClassification::Malformed => return None,
        FileUrlClassification::NotFileUrl => {}
    }
    if trimmed.contains("://") {
        return None;
    }

    let target_path = PathBuf::from(trimmed);
    if target_path.is_absolute() {
        // Absolute local imports are allowed by GTK and still need invalidation tracking
        return Some(target_path);
    }

    css_path.parent().map(|parent| parent.join(target_path))
}

fn canonical_or_resolved_path(path: &Path) -> Result<PathBuf> {
    // Canonical paths merge aliases when the target currently exists
    if let Ok(canonical_path) = fs::canonicalize(path) {
        return Ok(canonical_path);
    }
    if path.is_absolute() {
        return Ok(path.to_path_buf());
    }
    let current_dir = std::env::current_dir().context("resolve current directory for css path")?;
    Ok(current_dir.join(path))
}

pub(in super::super) fn hash_css_file_bytes(path: &Path) -> Result<String> {
    // Hash the exact bytes GTK would read so cached hits stay honest
    let (bytes, _metadata) = read_css_file_bounded(path)?;
    Ok(blake3::hash(&bytes).to_hex().to_string())
}

impl CssDependencyState {
    fn from_resolved_path(path: &Path) -> Result<Self> {
        let path_key = canonical_or_resolved_path(path)?;
        if !path.exists() {
            // Missing imports stay in the cache model so later creation invalidates the result
            return Ok(Self {
                path: path_key,
                identity: None,
                content_hash: None,
            });
        }
        // The descriptor read below remains authoritative if the visible path changes after this hint
        let (bytes, metadata) = read_css_file_bounded(path)?;

        Ok(Self {
            path: path_key,
            identity: Some(CssFileIdentity::from_metadata(&metadata)?),
            content_hash: Some(blake3::hash(&bytes).to_hex().to_string()),
        })
    }
}
