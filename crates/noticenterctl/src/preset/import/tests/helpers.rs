//! Test-only import helpers with deterministic prompts

pub(in crate::preset::import) use std::fs;
pub(in crate::preset::import) use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::Result;

pub(in crate::preset::import) use crate::preset::archive::write_bundle;
pub(in crate::preset::import) use crate::preset::config_root::{
    CollectedConfigFiles, PresetFileSource,
};
pub(in crate::preset::import) use crate::preset::export::flow::export_preset_from;
pub(in crate::preset::import) use crate::preset::manifest::{PresetManifest, PresetManifestFile};

use super::super::super::css_asset_refs::ExternalCssAssetRef;
use super::super::command::summary::{build_summary, ImportSummary};
use super::super::review::checks::ImportedExecContent;
use super::super::transaction::apply::{apply_import_plan, finalize_import_transaction};
use super::super::transaction::prepare::prepare_import;

static TEST_TEMP_COUNTER: AtomicUsize = AtomicUsize::new(0);

pub(in crate::preset::import) struct TempDirGuard {
    pub(in crate::preset::import) path: PathBuf,
}

impl TempDirGuard {
    pub(in crate::preset::import) fn new(name: &str) -> Self {
        // Unique temp roots keep import tests isolated from the live config tree
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock moved backwards")
            .as_nanos();
        let serial = TEST_TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("unixnotis-preset-import-{name}-{stamp}-{serial}"));
        fs::create_dir_all(&path).expect("create temp dir");
        Self { path }
    }

    pub(in crate::preset::import) fn write(&self, relative_path: &str, contents: &str) {
        let path = self.path.join(relative_path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create parent dirs");
        }
        fs::write(path, contents).expect("write test file");
    }
}

impl Drop for TempDirGuard {
    fn drop(&mut self) {
        // Cleanup is best effort because assertions should keep the original failure
        let _ = fs::remove_dir_all(&self.path);
    }
}

pub(in crate::preset::import) fn write_collected_bundle(
    root: &TempDirGuard,
    bundle_path: &Path,
    stamp: &str,
    files: &[(&str, &str)],
) {
    let collected = CollectedConfigFiles {
        files: files
            .iter()
            .map(|(relative_path, source_path)| {
                let source_path = root.path.join(source_path);
                PresetFileSource {
                    relative_path: PathBuf::from(relative_path),
                    size: fs::metadata(&source_path).expect("metadata").len(),
                    source_contents: fs::read(&source_path).expect("read source"),
                    source_path,
                    mode: 0o644,
                    contents_override: None,
                }
            })
            .collect(),
        ..Default::default()
    };
    let manifest = PresetManifest::new(
        "demo".to_string(),
        stamp.to_string(),
        env!("CARGO_PKG_VERSION").to_string(),
        collected
            .files
            .iter()
            .map(|file| PresetManifestFile {
                path: file.relative_path.display().to_string(),
                size: file.size,
            })
            .collect(),
    );
    write_bundle(bundle_path, &manifest, &collected).expect("write bundle");
}

pub(in crate::preset::import) fn import_preset_into(
    config_dir: &Path,
    input_path: &Path,
    except: &[String],
    dry_run: bool,
) -> Result<ImportSummary> {
    // Tests should stay deterministic even when `cargo test` owns a real terminal
    // Reuse the same shared import flow, but swap the prompt hooks for fixed answers
    import_preset_into_with_confirm(
        config_dir,
        input_path,
        except,
        dry_run,
        false,
        confirm_import_external_css_refs_for_tests,
        confirm_import_exec_content_for_tests,
    )
}

pub(in crate::preset::import) fn import_preset_into_with_confirm<F, G>(
    config_dir: &Path,
    input_path: &Path,
    except: &[String],
    dry_run: bool,
    allow_exec: bool,
    confirm_external_css_refs: F,
    confirm_exec_content: G,
) -> Result<ImportSummary>
where
    F: FnOnce(&[ExternalCssAssetRef]) -> Result<()>,
    G: FnOnce(&ImportedExecContent, bool) -> Result<()>,
{
    // Tests inject a fixed answer here so the import plan can be checked without terminal prompts
    let prepared = prepare_import(
        config_dir,
        input_path,
        except,
        allow_exec,
        confirm_external_css_refs,
        confirm_exec_content,
    )?;

    if dry_run {
        // Dry-run reports the exact write plan without creating backups or files
        return Ok(build_summary(&prepared.plan, None, true));
    }

    // Test helpers do not run css-check, but they still use the same staged apply and commit flow
    let transaction = apply_import_plan(config_dir, &prepared.plan)?;
    let backup_dir = finalize_import_transaction(transaction)?;
    Ok(build_summary(&prepared.plan, backup_dir, false))
}

fn confirm_import_external_css_refs_for_tests(external_refs: &[ExternalCssAssetRef]) -> Result<()> {
    // Most tests do not care about the warning path, so empty input should stay quiet
    if external_refs.is_empty() {
        return Ok(());
    }

    // Test runs should fail fast instead of waiting for a terminal answer
    let details = super::super::review::prompts::format_external_css_ref_lines(external_refs);
    Err(anyhow::anyhow!(
        "preset import found CSS asset references that leave the UnixNotis config directory or use remote URLs; rerun interactively to confirm anyway\n{}",
        details.join("\n")
    ))
}

fn confirm_import_exec_content_for_tests(
    exec_content: &ImportedExecContent,
    allow_exec: bool,
) -> Result<()> {
    // Explicit trust should keep the shared helper aligned with the real import path
    if allow_exec {
        return Ok(());
    }

    // Empty bundles should stay on the normal import path
    if exec_content.commands.is_empty() && exec_content.files.is_empty() {
        return Ok(());
    }

    // Test runs should surface the same guidance every time instead of prompting
    Err(anyhow::anyhow!(
        "preset import found executable commands or bundled scripts; rerun interactively to inspect them or use --allow-exec only if the preset is trusted"
    ))
}
