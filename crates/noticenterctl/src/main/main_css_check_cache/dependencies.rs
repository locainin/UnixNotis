use anyhow::{Context, Result};
use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use super::super::main_css_check_parse::strip_css_comments;
use super::model::{CssDependencyState, CssFileIdentity};

pub(in super::super) fn collect_import_dependency_states(
    css_path: &Path,
) -> Result<Vec<CssDependencyState>> {
    let mut dependencies = BTreeMap::new();
    let mut visited = HashSet::new();
    visited.insert(canonical_or_resolved_path(css_path)?);
    collect_import_dependency_states_from(css_path, &mut visited, &mut dependencies)?;
    Ok(dependencies.into_values().collect())
}

fn collect_import_dependency_states_from(
    css_path: &Path,
    visited: &mut HashSet<PathBuf>,
    dependencies: &mut BTreeMap<String, CssDependencyState>,
) -> Result<()> {
    let contents =
        fs::read_to_string(css_path).with_context(|| format!("read css file {}", css_path.display()))?;

    for import_path in imported_css_paths(&contents, css_path) {
        let dependency = CssDependencyState::from_resolved_path(&import_path)?;
        let dependency_key = dependency.path.clone();
        let dependency_sort_key = dependency_key.to_string_lossy().into_owned();
        let recurse = dependency.identity.is_some();

        if !visited.insert(dependency_key) {
            continue;
        }

        dependencies.entry(dependency_sort_key).or_insert(dependency);

        if recurse {
            collect_import_dependency_states_from(&import_path, visited, dependencies)?;
        }
    }

    Ok(())
}

fn imported_css_paths(contents: &str, css_path: &Path) -> Vec<PathBuf> {
    let stripped = strip_css_comments(contents);
    let mut paths = Vec::new();
    let mut cursor = 0usize;

    while let Some(import_offset) = stripped[cursor..].find("@import") {
        let import_start = cursor + import_offset;
        let statement_start = import_start + "@import".len();
        let Some(statement_end_offset) = stripped[statement_start..].find(';') else {
            break;
        };
        let statement_end = statement_start + statement_end_offset;
        let statement = stripped[statement_start..statement_end].trim();

        if let Some(path) = import_target_path(statement, css_path) {
            paths.push(path);
        }

        cursor = statement_end + 1;
    }

    paths
}

fn import_target_path(statement: &str, css_path: &Path) -> Option<PathBuf> {
    let target = parse_import_target(statement)?;
    resolve_import_target(css_path, &target)
}

fn parse_import_target(statement: &str) -> Option<String> {
    let trimmed = statement.trim_start();
    if let Some(rest) = trimmed.strip_prefix("url(") {
        let closing = rest.find(')')?;
        return Some(unquote_import_target(rest[..closing].trim()).to_string());
    }

    let quote = trimmed.chars().next()?;
    if quote != '"' && quote != '\'' {
        return None;
    }

    let end = trimmed[1..].find(quote)?;
    Some(trimmed[1..1 + end].to_string())
}

fn unquote_import_target(value: &str) -> &str {
    let bytes = value.as_bytes();
    if bytes.len() >= 2
        && ((bytes[0] == b'"' && bytes[bytes.len() - 1] == b'"')
            || (bytes[0] == b'\'' && bytes[bytes.len() - 1] == b'\''))
    {
        &value[1..value.len() - 1]
    } else {
        value
    }
}

fn resolve_import_target(css_path: &Path, target: &str) -> Option<PathBuf> {
    let trimmed = target.trim();
    if trimmed.is_empty() {
        return None;
    }
    if trimmed.starts_with("//")
        || trimmed.starts_with("http:")
        || trimmed.starts_with("https:")
        || trimmed.starts_with("data:")
    {
        return None;
    }
    if let Some(path) = local_file_url_path(trimmed) {
        return Some(path);
    }
    if trimmed.contains("://") {
        return None;
    }

    let target_path = PathBuf::from(trimmed);
    if target_path.is_absolute() {
        return Some(target_path);
    }

    css_path.parent().map(|parent| parent.join(target_path))
}

fn local_file_url_path(value: &str) -> Option<PathBuf> {
    let path = value.strip_prefix("file://")?;
    let path = path.strip_prefix("localhost/").unwrap_or(path);
    if !path.starts_with('/') {
        return None;
    }
    Some(PathBuf::from(path))
}

fn canonical_or_resolved_path(path: &Path) -> Result<PathBuf> {
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
    let bytes = fs::read(path).with_context(|| format!("read css file {}", path.display()))?;
    Ok(blake3::hash(&bytes).to_hex().to_string())
}

impl CssDependencyState {
    fn from_resolved_path(path: &Path) -> Result<Self> {
        let path_key = canonical_or_resolved_path(path)?;
        let Some(metadata) = fs::metadata(path).ok() else {
            return Ok(Self {
                path: path_key,
                identity: None,
                content_hash: None,
            });
        };

        Ok(Self {
            path: path_key,
            identity: Some(CssFileIdentity::from_metadata(&metadata)?),
            content_hash: Some(hash_css_file_bytes(path)?),
        })
    }
}
