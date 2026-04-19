use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

use super::dependencies::{collect_import_dependency_states, hash_css_file_bytes};
use super::model::{CachedParseDiagnostic, CssFileIdentity, CssParseReport, CssParseWorkItem};
use super::parse::{parse_css_file_with_gtk, render_cached_diagnostics};
use super::store::{default_css_parse_cache_path, CssParseCacheState};

pub(in super::super) fn validate_css_parse_files(
    files: &[PathBuf],
    config_dir: &Path,
    display_root: &str,
) -> Result<CssParseReport> {
    // Work items lock in identity before any parser callbacks run
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
    // Cache state is optional so tests can inject a fixed path or skip persistence
    let mut cache = cache_path.map(|path| CssParseCacheState::load(path.to_path_buf()));
    let mut diagnostics = Vec::new();
    let mut error_count = 0usize;

    for work_item in work_items {
        // Cached hits still get rendered into fresh user-facing diagnostics
        if let Some(cached_diagnostics) = cache
            .as_ref()
            .map(|cache| cache.lookup(work_item))
            .transpose()?
            .flatten()
        {
            let cached_diagnostics = render_cached_diagnostics(
                cached_diagnostics,
                work_item,
                config_dir,
                display_root,
            );
            error_count += cached_diagnostics.len();
            diagnostics.extend(cached_diagnostics);
            continue;
        }

        // Cache misses always go through the same parse path as a cold run
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
            content_hash: hash_css_file_bytes(path)?,
            dependencies: collect_import_dependency_states(path)?,
        });
    }
    Ok(work_items)
}

#[cfg(test)]
pub(in super::super) fn validate_css_parse_files_with(
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
pub(in super::super) fn parse_diagnostic_for_test(
    message: impl Into<String>,
) -> Vec<CachedParseDiagnostic> {
    // Tests only need one stable top-level parser finding shape
    vec![CachedParseDiagnostic {
        source: super::model::CachedDiagnosticSource::TopLevel,
        line: Some(1),
        column: Some(1),
        message: message.into(),
        hint: None,
    }]
}
