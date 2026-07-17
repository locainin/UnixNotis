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

pub(in super::super) fn run_cached_parse_session<F>(
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
            .as_mut()
            .map(|cache| cache.lookup(work_item))
            .transpose()?
            .flatten()
        {
            // A cache hit is usable only while the live path still matches its captured key
            let current = build_parse_work_item(&work_item.load_path)?;
            if current == *work_item {
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
        }

        let mut current = build_parse_work_item(&work_item.load_path)?;
        let mut completed = false;
        for attempt in 0..2 {
            // The helper may read a live path, so its result needs a matching post-parse snapshot
            let fresh_diagnostics = parse_file(&current)?;
            let after_parse = build_parse_work_item(&current.load_path)?;
            if after_parse == current {
                error_count += fresh_diagnostics.len();
                diagnostics.extend(render_cached_diagnostics(
                    &fresh_diagnostics,
                    &current,
                    config_dir,
                    display_root,
                ));
                if let Some(cache) = cache.as_mut() {
                    cache.store(&current, fresh_diagnostics)?;
                }
                completed = true;
                break;
            }

            if attempt == 0 {
                // One retry handles ordinary editor replace-and-rename saves without polling
                current = after_parse;
            }
        }

        if !completed {
            let display_path = super::super::files::format_display_path(
                config_dir,
                display_root,
                &current.load_path,
            );
            diagnostics.push(super::super::report::CssCheckDiagnostic::warning(
                super::super::report::CssCheckCategory::Parse,
                display_path,
                "stylesheet changed repeatedly during validation; run css-check again after edits settle",
            ));
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

pub(in super::super) fn build_parse_work_items(files: &[PathBuf]) -> Result<Vec<CssParseWorkItem>> {
    let mut work_items = Vec::with_capacity(files.len());
    for path in files {
        work_items.push(build_parse_work_item(path)?);
    }
    Ok(work_items)
}

fn build_parse_work_item(path: &Path) -> Result<CssParseWorkItem> {
    // Metadata should come from the real target, not the symlink shell
    let metadata =
        fs::metadata(path).with_context(|| format!("read css metadata {}", path.display()))?;
    let canonical_path =
        fs::canonicalize(path).with_context(|| format!("resolve css file {}", path.display()))?;
    Ok(CssParseWorkItem {
        load_path: path.to_path_buf(),
        canonical_path,
        identity: CssFileIdentity::from_metadata(&metadata)?,
        content_hash: hash_css_file_bytes(path)?,
        dependencies: collect_import_dependency_states(path)?,
    })
}
