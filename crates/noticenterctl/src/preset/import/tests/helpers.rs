//! Test-only import helpers with deterministic prompts

use std::path::Path;

use anyhow::{anyhow, Result};

use super::super::css_asset_refs::ExternalCssAssetRef;
use super::apply::{apply_import_plan, finalize_import_transaction};
use super::checks::ImportedExecContent;
use super::prepare::prepare_import;
use super::summary::{build_summary, ImportSummary};

pub(super) fn import_preset_into(
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

pub(super) fn import_preset_into_with_confirm<F, G>(
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
    let details = super::prompts::format_external_css_ref_lines(external_refs);
    Err(anyhow!(
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
    Err(anyhow!(
        "preset import found executable commands or bundled scripts; rerun interactively to inspect them or use --allow-exec only if the preset is trusted"
    ))
}
